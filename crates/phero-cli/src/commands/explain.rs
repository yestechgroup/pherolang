//! `pherolang explain`: print one diagnostic code's registry entry (band,
//! meaning, fix), or list the allocation ledger and every registered
//! code. Codes resolve from the central `phero-diagnostics` registry —
//! numeric surface codes and the driver's camelCase LSP codes alike.

use std::process::ExitCode;

pub(crate) fn run(code: Option<&str>, list: bool) -> anyhow::Result<ExitCode> {
    if list {
        print_list();
        return Ok(ExitCode::SUCCESS);
    }
    let Some(code) = code else {
        anyhow::bail!(
            "no code given — pass a diagnostic code (e.g. `pherolang explain E0104`) or `--list`"
        );
    };
    let Some(entry) = phero_diagnostics::lookup(code) else {
        eprintln!(
            "unknown diagnostic code '{code}' — run `pherolang explain --list` to see every registered code"
        );
        return Ok(ExitCode::FAILURE);
    };
    let band = phero_diagnostics::ledger()
        .iter()
        .find(|band| band.id == entry.band)
        .expect("every entry names a registered band");
    println!(
        "{} — {} ({})",
        entry.code,
        entry.title,
        entry.severity.as_str()
    );
    println!("band: {} — {}", band.id, band.owner);
    println!("{}", entry.meaning);
    println!("fix: {}", entry.fix);
    Ok(ExitCode::SUCCESS)
}

fn print_list() {
    println!("pherolang diagnostic codes — `pherolang explain <code>` prints one entry");
    println!();
    let bands = phero_diagnostics::ledger();
    let id_width = bands.iter().map(|band| band.id.len()).max().unwrap_or(0);
    let range_width = bands.iter().map(|band| band.range.len()).max().unwrap_or(0);
    println!(
        "{:<id_width$}  {:<range_width$}  {:<8}  NEXT FREE",
        "BAND", "RANGE", "SEVERITY"
    );
    for band in bands {
        println!(
            "{:<id_width$}  {:<range_width$}  {:<8}  {}",
            band.id, band.range, band.severity_note, band.next_free
        );
    }
    println!();
    let entries = phero_diagnostics::all();
    let code_width = entries
        .iter()
        .map(|entry| entry.code.len())
        .max()
        .unwrap_or(0);
    for entry in entries {
        println!(
            "{:<code_width$}  {:<7}  {}",
            entry.code,
            entry.severity.as_str(),
            entry.title
        );
    }
}
