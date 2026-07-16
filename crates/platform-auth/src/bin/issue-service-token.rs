#![forbid(unsafe_code)]

fn usage() -> ! {
    eprintln!(
        "usage: issue-service-token <service-id> <audience> <scope[,scope...]> [ttl-seconds]"
    );
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let service_id = args.next().unwrap_or_else(|| usage());
    let audience = args.next().unwrap_or_else(|| usage());
    let scope_list = args.next().unwrap_or_else(|| usage());
    let ttl_seconds = args
        .next()
        .map(|value| value.parse::<i64>().unwrap_or_else(|_| usage()))
        .unwrap_or(86_400);
    if args.next().is_some() {
        usage();
    }
    let scopes = scope_list
        .split(',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .collect::<Vec<_>>();
    if scopes.is_empty() {
        usage();
    }

    let config = platform_auth::AuthConfig::from_env();
    if let Err(error) = config.validate_for_environment("production") {
        eprintln!("invalid issuer configuration: {error}");
        std::process::exit(1);
    }
    match platform_auth::generate_service_token(
        &config,
        &service_id,
        &audience,
        &scopes,
        ttl_seconds,
    ) {
        Ok(token) => println!("{token}"),
        Err(error) => {
            eprintln!("failed to issue service credential: {error}");
            std::process::exit(1);
        }
    }
}
