//! Smallest native program around the library (for the size report): stdin lines in, normalised lines out.

use std::io::BufRead;

fn main() {
    for line in std::io::stdin().lock().lines() {
        println!("{}", nrmlz::normalize_de(&line.unwrap()));
    }
}
