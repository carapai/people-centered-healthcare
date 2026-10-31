//! Builds the breached-password filter (`data/breached-passwords.bloom`) from a password list,
//! one password per line:
//!
//!   cargo run -p health-auth --example breached_filter -- <list.txt> <out.bloom>

use health_auth::BreachedPasswords;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        return Err("usage: breached_filter <list.txt> <out.bloom>".into());
    };
    let text = std::fs::read_to_string(input)?;
    let mut passwords: Vec<&str> = text
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.is_empty())
        .collect();
    passwords.sort_unstable();
    passwords.dedup();
    let filter = BreachedPasswords::build(passwords.iter().copied(), 0.001);
    std::fs::write(output, filter.to_bytes())?;
    std::io::Write::write_all(
        &mut std::io::stdout(),
        format!("{} passwords\n", filter.len()).as_bytes(),
    )?;
    Ok(())
}
