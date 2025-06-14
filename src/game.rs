use std::collections::{HashMap, HashSet};

use anyhow::Context;
use derive_new::new;
use rayon::prelude::*;

type Letter = char;
type Word = String;
type Points = usize;

#[derive(Debug, new)]
pub struct Game {
    center_letter: Letter,
    non_center_letters: Vec<Letter>,
}

// invariant: center letter is not contained within non center letters.
pub struct GameProcessed {
    center_letter: Letter,
    non_center_letters: HashSet<Letter>,
}

impl GameProcessed {
    pub fn letter_count(&self) -> usize {
        return self.non_center_letters.len() + 1;
    }
}

impl TryFrom<&Game> for GameProcessed {
    type Error = anyhow::Error;

    fn try_from(game: &Game) -> Result<Self, Self::Error> {
        anyhow::ensure!(
            game.non_center_letters
                .iter()
                .find(|&&letter| letter == game.center_letter)
                .is_none(),
            "center letter may not be part of non center letters"
        );
        Ok(GameProcessed {
            center_letter: game.center_letter,
            non_center_letters: game
                .non_center_letters
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
        })
    }
}

struct Guess<'a> {
    guessed_word: &'a Word,
}

enum GuessingError {
    TooShort,
    UnknownWord,
    DisallowedLetter(Letter),
    MissingCenterLetter,
}

impl<'a> Guess<'a> {
    fn new(word: &'a String) -> Guess<'a> {
        Guess { guessed_word: word }
    }

    fn eval_points(
        &self,
        game: &GameProcessed,
        dict: &Dictionary,
    ) -> Result<Points, GuessingError> {
        // Rules:
        // - Words must contain at least 4 letters.
        // - Words must include the center letter.
        // - Our word list does not include words that are obscure, hyphenated, or proper nouns.
        // - No cussing either, sorry.
        // - Letters can be used more than once.

        if self.guessed_word.len() < 4 {
            return Err(GuessingError::TooShort);
        }
        if !dict.words.contains(self.guessed_word) {
            return Err(GuessingError::UnknownWord);
        }

        let mut center_letter_count = 0;
        let mut guessed_letters = HashSet::new();
        for c in self.guessed_word.chars() {
            if c == game.center_letter {
                guessed_letters.insert(c);
                center_letter_count += 1;
            } else if game.non_center_letters.contains(&c) {
                guessed_letters.insert(c);
            } else {
                return Err(GuessingError::DisallowedLetter(c));
            }
        }

        if center_letter_count == 0 {
            return Err(GuessingError::MissingCenterLetter);
        }

        let is_pangram = game.letter_count() == guessed_letters.len();

        // How points are awarded:
        // 4-letter words are worth 1 point each.
        // Longer words earn 1 point per letter (this spec is slightly unclear here, but should be interpreted as: "earn 1 **extra** point for every letter other than the first 4").
        // Each puzzle includes at least one "pangram" which uses every letter. These are worth 7 extra points!
        let points = 1 + (self.guessed_word.len() - 4) + (if is_pangram { 7 } else { 0 });

        Ok(points)
    }
}

const WORD_LIST_URL: &str =
    "https://raw.githubusercontent.com/rressler/data_raw_courses/main/scrabble_words.txt";

pub struct Dictionary {
    // TODO: remove pub
    pub words: HashSet<Word>,
}

impl Dictionary {
    pub fn scrape() -> anyhow::Result<Dictionary> {
        let response = reqwest::blocking::get(WORD_LIST_URL)
            .with_context(|| format!("failed to GET {}", WORD_LIST_URL))?
            .error_for_status()?
            .text()
            .context("failed to read response body as text")?;

        let words: HashSet<String> = response
            .lines()
            // filter out non-word lines: only keep non-empty lines with only uppercase chars.
            .filter(|line| !line.is_empty() && line.chars().all(char::is_uppercase))
            // filter out short words.
            .filter(|line| line.len() >= 4)
            .map(str::trim)
            .map(|line| line.to_string())
            .collect();

        Ok(Dictionary { words })
    }
}

#[derive(Debug)]
pub struct GameResult<'a> {
    word_to_points: HashMap<&'a Word, Points>,
}

pub trait SolveStrategy<'a> {
    fn new(dict: &'a Dictionary) -> Self;

    fn solve(&self, game: &GameProcessed) -> GameResult<'a>;
}

pub struct GameSolver<S> {
    strategy: S,
}

impl<'a, S> GameSolver<S>
where
    S: SolveStrategy<'a>,
{
    pub fn new(dict: &'a Dictionary) -> Self {
        let strategy = S::new(dict);
        GameSolver { strategy }
    }

    pub fn solve(&self, game: &Game) -> anyhow::Result<GameResult<'a>> {
        let processed: GameProcessed = game.try_into()?;
        Ok(self.strategy.solve(&processed))
    }
}

pub struct BruteForce<'a> {
    dict: &'a Dictionary,
}

impl<'a> SolveStrategy<'a> for BruteForce<'a> {
    fn new(dict: &'a Dictionary) -> Self {
        BruteForce { dict }
    }

    fn solve(&self, game: &GameProcessed) -> GameResult<'a> {
        let word_to_points = self
            .dict
            .words
            .iter()
            .filter_map(|word| {
                Guess::new(word)
                    .eval_points(game, self.dict)
                    .ok()
                    .map(|points| (word, points))
            })
            .collect();

        GameResult { word_to_points }
    }
}

pub struct ParallelBruteForce<'a> {
    dict: &'a Dictionary,
}

impl<'a> SolveStrategy<'a> for ParallelBruteForce<'a> {
    fn new(dict: &'a Dictionary) -> Self {
        ParallelBruteForce { dict }
    }

    fn solve(&self, game: &GameProcessed) -> GameResult<'a> {
        let word_to_points = self
            .dict
            .words
            .par_iter()
            .filter_map(|word| {
                Guess::new(word)
                    .eval_points(game, self.dict)
                    .ok()
                    .map(|points| (word, points))
            })
            .collect();

        GameResult { word_to_points }
    }
}

// Pre-compute a map from letter to all words with that letter.
pub struct LetterMap<'a> {
    letter_to_words: HashMap<Letter, HashSet<&'a Word>>,
    dict: &'a Dictionary,
}

impl<'a> SolveStrategy<'a> for LetterMap<'a> {
    fn new(dict: &'a Dictionary) -> Self {
        let mut letter_to_words = HashMap::new();
        for word in &dict.words {
            for letter in word.chars() {
                letter_to_words
                    .entry(letter)
                    .or_insert_with(|| HashSet::new())
                    .insert(word);
            }
        }
        Self {
            letter_to_words,
            dict,
        }
    }

    fn solve(&self, game: &GameProcessed) -> GameResult<'a> {
        let word_to_points = self
            .letter_to_words
            .get(&game.center_letter)
            .into_iter()
            .flatten()
            .filter_map(|&word| {
                Guess::new(word)
                    .eval_points(game, self.dict)
                    .ok()
                    .map(|points| (word, points))
            })
            .collect();

        GameResult { word_to_points }
    }
}

pub struct ParallelLetterMap<'a> {
    letter_to_words: HashMap<Letter, Vec<&'a Word>>,
    dict: &'a Dictionary,
}

impl<'a> SolveStrategy<'a> for ParallelLetterMap<'a> {
    fn new(dict: &'a Dictionary) -> Self {
        let mut letter_to_words = HashMap::new();
        for word in &dict.words {
            for letter in word.chars() {
                letter_to_words
                    .entry(letter)
                    .or_insert_with(|| Vec::new())
                    .push(word);
            }
        }
        Self {
            letter_to_words,
            dict,
        }
    }

    fn solve(&self, game: &GameProcessed) -> GameResult<'a> {
        let word_to_points = match self.letter_to_words.get(&game.center_letter) {
            Some(words) => words
                .par_iter()
                .filter_map(|&word| {
                    Guess::new(word)
                        .eval_points(game, self.dict)
                        .ok()
                        .map(|points| (word, points))
                })
                .collect(),
            None => HashMap::new(),
        };

        GameResult { word_to_points }
    }
}
