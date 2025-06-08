use game::{BruteForce, Dictionary, Game, GameSolver, ParallelBruteForce};

mod game;

fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let dict = timeit!("scrape dictionary", Dictionary::scrape()?);
    log::info!("dictionary had {} entries", dict.words.len());

    let game = Game::new('C', vec!['A', 'L', 'T', 'E', 'F', 'I']);
    dbg!(timeit!(
        "brute force",
        GameSolver::solve_with::<BruteForce>(&game, &dict)?
    ));
    dbg!(timeit!(
        "parallel brute force",
        GameSolver::solve_with::<ParallelBruteForce>(&game, &dict)?
    ));

    Ok(())
}

#[macro_export]
macro_rules! timeit {
    // bare expression
    ($label:expr, $expr:expr) => {{
        let __t_start = std::time::Instant::now();
        let __t_val = $expr;
        let __t_dur = __t_start.elapsed();
        log::info!(concat!("[timeit] '{}' took {:?}"), $label, __t_dur);
        __t_val
    }};
    // block `{ ... }`
    ($label:expr, { $($body:tt)* }) => {{
        let __t_start = std::time::Instant::now();
        let __t_val = { $($body)* };
        let __t_dur = __t_start.elapsed();
        log::info!(concat!("[timeit] '{}' took {:?}"), $label, __t_dur);
        __t_val
    }};
}
