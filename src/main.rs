use game::{Dictionary, Game, GameSolver};

mod game;

fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    log::info!("started scraping dictionary");
    let dict = Dictionary::scrape()?;
    log::info!(
        "finished loading dictionary with {} entries",
        dict.words.len()
    );

    let game = Game::new('C', vec!['A', 'L', 'T', 'E', 'F', 'I']);
    let game_result = GameSolver::solve(&game, &dict)?;
    log::info!("finished solving game");
    dbg!(game_result);

    Ok(())
}
