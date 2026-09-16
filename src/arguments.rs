use crate::devcontainer::Agent;
use crate::errors::{AgentWorktreeError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommand {
    Destroy,
    Doctor,
    List,
    Run,
    Setup,
    Stop,
}

impl CliCommand {
    fn parse(value: &str) -> Option<CliCommand> {
        match value {
            "destroy" => Some(CliCommand::Destroy),
            "doctor" => Some(CliCommand::Doctor),
            "list" => Some(CliCommand::List),
            "run" => Some(CliCommand::Run),
            "setup" => Some(CliCommand::Setup),
            "stop" => Some(CliCommand::Stop),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CliCommand::Destroy => "destroy",
            CliCommand::Doctor => "doctor",
            CliCommand::List => "list",
            CliCommand::Run => "run",
            CliCommand::Setup => "setup",
            CliCommand::Stop => "stop",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub agent: Agent,
    pub agent_args: Vec<String>,
    pub base: Option<String>,
    pub command: CliCommand,
    pub help: bool,
    pub json: bool,
    pub name: Option<String>,
    pub version: bool,
    pub yes: bool,
}

fn agent_value(value: Option<String>, default_agent: Agent) -> Result<Agent> {
    match value {
        None => Ok(default_agent),
        Some(raw) => Agent::parse(&raw).ok_or_else(|| AgentWorktreeError::usage("--agent must be codex or claude")),
    }
}

struct StringOptions {
    agent: Option<String>,
    base: Option<String>,
    name: Option<String>,
}

struct ParsedArgs {
    positionals: Vec<String>,
    strings: StringOptions,
    help: bool,
    json: bool,
    version: bool,
    yes: bool,
}

fn parse_wrapper_args(args: &[String]) -> Result<ParsedArgs> {
    let mut positionals = Vec::new();
    let mut agent = None;
    let mut base = None;
    let mut name = None;
    let mut help = false;
    let mut json = false;
    let mut version = false;
    let mut yes = false;

    let mut i = 0;
    while i < args.len() {
        let token = args[i].as_str();
        if let Some(rest) = token.strip_prefix("--") {
            let (flag_name, inline_value) = match rest.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (rest, None),
            };
            let take_value = |i: &mut usize| -> Result<String> {
                if let Some(value) = &inline_value {
                    Ok(value.clone())
                } else {
                    *i += 1;
                    args.get(*i)
                        .cloned()
                        .ok_or_else(|| AgentWorktreeError::usage(format!("--{flag_name} requires a value")))
                }
            };
            match flag_name {
                "agent" => agent = Some(take_value(&mut i)?),
                "base" => base = Some(take_value(&mut i)?),
                "name" => name = Some(take_value(&mut i)?),
                "help" => help = true,
                "json" => json = true,
                "version" => version = true,
                "yes" => yes = true,
                other => return Err(AgentWorktreeError::usage(format!("unknown option '--{other}'"))),
            }
        } else if token == "-h" {
            help = true;
        } else if token == "-y" {
            yes = true;
        } else if token.starts_with('-') && token.len() > 1 && !token.chars().nth(1).unwrap().is_ascii_digit() {
            return Err(AgentWorktreeError::usage(format!("unknown option '{token}'")));
        } else {
            positionals.push(token.to_string());
        }
        i += 1;
    }

    Ok(ParsedArgs { positionals, strings: StringOptions { agent, base, name }, help, json, version, yes })
}

pub fn parse_arguments(argv: &[String], default_agent: Agent) -> Result<CliOptions> {
    let separator_index = argv.iter().position(|arg| arg == "--");
    let wrapper_args: Vec<String> = match separator_index {
        Some(index) => argv[..index].to_vec(),
        None => argv.to_vec(),
    };
    let agent_args: Vec<String> = match separator_index {
        Some(index) => argv[index + 1..].to_vec(),
        None => Vec::new(),
    };

    let first = wrapper_args.first();
    let command = first.and_then(|value| CliCommand::parse(value)).unwrap_or(CliCommand::Run);
    let args: Vec<String> = if first.is_some() && CliCommand::parse(first.unwrap()) == Some(command) {
        wrapper_args[1..].to_vec()
    } else {
        wrapper_args.clone()
    };

    let parsed = parse_wrapper_args(&args)?;
    if parsed.positionals.len() > 1 {
        return Err(AgentWorktreeError::usage("expected at most one lane name"));
    }
    let positional_name = parsed.positionals.first().cloned();
    if positional_name.is_some() && parsed.strings.name.is_some() {
        return Err(AgentWorktreeError::usage("provide the lane name once"));
    }
    let name = parsed.strings.name.or(positional_name);
    if command != CliCommand::Run && !agent_args.is_empty() {
        return Err(AgentWorktreeError::usage("arguments after -- are only valid with run"));
    }

    Ok(CliOptions {
        agent: agent_value(parsed.strings.agent, default_agent)?,
        agent_args,
        base: parsed.strings.base,
        command,
        help: parsed.help,
        json: parsed.json,
        name,
        version: parsed.version,
        yes: parsed.yes,
    })
}
