use chrono::Local;

#[macro_use]
extern crate log;

struct ConsoleLogger;

impl log::Log for ConsoleLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        println!(
            "{} {} {} - {}",
            Local::now().format("%Y-%m-%dT%H:%M:%S%z"),
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

static LOGGER: ConsoleLogger = ConsoleLogger;

fn main() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Debug);

    info!("Starting organiseFS...");
}
