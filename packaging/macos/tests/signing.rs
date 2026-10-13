fn main() {
    println!(
        "{}",
        if cfg!(second_build) {
            "second"
        } else {
            "first"
        }
    );
}
