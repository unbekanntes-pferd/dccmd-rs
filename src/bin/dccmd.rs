#[tokio::main]
async fn main() {
    std::process::exit(dccmd_rs::run().await);
}
