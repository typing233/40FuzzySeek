use crate::Cli;

pub struct Config {
    pub multi_select: bool,
    pub preview_cmd: Option<String>,
    pub initial_query: String,
    #[allow(dead_code)]
    pub height: u16,
}

impl Config {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            multi_select: cli.multi,
            preview_cmd: cli.preview.clone(),
            initial_query: cli.query.clone(),
            height: cli.height,
        }
    }
}
