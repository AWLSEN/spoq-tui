//! Test the GitHub CLI authentication flow.
//!
//! Run with: cargo run --example gh_auth_test

use spoq::setup::gh_auth::{ensure_gh_authenticated, is_gh_authenticated, is_gh_installed};

fn main() {
    println!("=== GitHub CLI Auth Test ===\n");

    // Check current status
    println!("Checking current status...");
    println!("  gh installed: {}", is_gh_installed());
    println!("  gh authenticated: {}", is_gh_authenticated());
    println!();

    // Run the full auth flow
    println!("Running ensure_gh_authenticated()...\n");

    match ensure_gh_authenticated() {
        Ok(()) => {
            println!("\n=== SUCCESS ===");
            println!("GitHub CLI is installed and authenticated!");
        }
        Err(e) => {
            println!("\n=== FAILED ===");
            println!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
