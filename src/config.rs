use log_derive::logfn;
use colored::Colorize;
use std::collections::HashMap;
use std::fs::File;
use std::hash::{Hash, Hasher};

use crate::error::ArchonError;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PocChain {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    pub priority: u8,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_bhd: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_pool: Option<bool>,

    pub url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeric_id_to_passphrase: Option<HashMap<u64, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeric_id_to_target_deadline: Option<HashMap<u64, u64>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub historical_rounds: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_deadline: Option<u64>,

    pub color: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub get_mining_info_interval: Option<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_dynamic_deadlines: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_lower_block_heights: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub requeue_interrupted_blocks: Option<bool>,
}

impl PartialEq for PocChain {
    fn eq(&self, other: &PocChain) -> bool {
        self.name == other.name
            && self.enabled == other.enabled
            && self.priority == other.priority
            && self.is_bhd == other.is_bhd
            && self.is_pool == other.is_pool
            && self.url == other.url
            && self.historical_rounds == other.historical_rounds
            && self.target_deadline == other.target_deadline
            && self.color == other.color
            && self.get_mining_info_interval == other.get_mining_info_interval
            && self.use_dynamic_deadlines == other.use_dynamic_deadlines
            && self.allow_lower_block_heights == other.allow_lower_block_heights
    }
}
impl Eq for PocChain {}

impl Hash for PocChain {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.enabled.hash(state);
        self.priority.hash(state);
        self.is_bhd.hash(state);
        self.is_pool.hash(state);
        self.url.hash(state);
        self.historical_rounds.hash(state);
        self.target_deadline.hash(state);
        self.color.hash(state);
        self.get_mining_info_interval.hash(state);
        self.use_dynamic_deadlines.hash(state);
        self.allow_lower_block_heights.hash(state);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub grace_period: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_mode: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt_lower_priority_blocks: Option<bool>,

    pub web_server_bind_address: String,
    pub web_server_port: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_poc_chain_colors: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub poc_chains: Option<Vec<PocChain>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub outage_status_update_interval: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_plots_size_in_tebibytes: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_plots_size_in_gibibytes: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_plots_size_in_terabytes: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_plots_size_in_gigabytes: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_human_readable_deadlines: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask_account_ids_in_console: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_24_hour_time: Option<bool>,
}

impl Config {
    pub fn create_default() -> String {
        return r#"---
# Grace Period: How long (in seconds) Archon will let blocks mine for - Make this at least as long as your maximum scan times.
gracePeriod: 20

# Priority Mode: Optional. Default: True.
#   TRUE: Chains will be mined in the order specified in the chain configurations below.
#  FALSE: Blocks will be mined on a first in, first out basis.
priorityMode: true

# Interrupt Lower Priority Blocks: Optional. Default: True. Only used in priority mode.
#   TRUE: Lower priority blocks will be interrupted by new blocks from a higher priority chain.
#  FALSE: Blocks will not be interrupted unless outdated by a new block from the same chain.
interruptLowerPriorityBlocks: true

# Web Server Bind Address: Which interface to listen for requests from your miners and/or web requests.
# Localhost only - will only listen for requests made from the same machine
#webServerBindAddress: 127.0.0.1
# LAN IP - Will listen for requests made over your local network to this machine, if this machines LAN IP = 192.168.1.1
#webServerBindAddress: 192.168.1.1
# Universal - bind to all interfaces
webServerBindAddress: 0.0.0.0

# Web Server Port: Listen for requests on this port.
webServerPort: 1337

# Use PoC Chain Colors: Optional. Default: True. Whether to use colors in console logging for each chain.
# NOTE: On windows, if your colors are dim, see: https://blogs.msdn.microsoft.com/commandline/2017/08/02/updating-the-windows-console-colors/
usePocChainColors: true

# Outage Status Update Interval: Optional. Interval (in seconds) in which to post logs about outages. Default = 300 seconds (5 minutes).
#   The lower this number, the more error messages about outages you will see in the event of an outage.
outageStatusUpdateInterval: 300

# Total Plots Size In (Unit): These are used for calculating dynamic deadlines, different units are provided for convenience.
# These options are all optional, if more than one is specified, they will be *ADDED TOGETHER*, so don't fill each one out with your total plots size! Eg: 10 TiB + 3 TB + 1024 GiB + 8000 GB = 21.0044417195022106170654296875 TiB
totalPlotsSizeInTebibytes: 10    # 10 TiB
#totalPlotsSizeInTerabytes: 3     # 3 TB (2.72 TiB)
#totalPlotsSizeInGibibytes: 1024  # 1024 GiB (1 TiB)
#totalPlotsSizeInGigabytes: 8000  # 8000 GB (7.27 TiB)

# Show Human Readable Deadlines: Optional. If true, values displayed in seconds will be appended with a human readable value, for example: 3345951 (1m 8d 17:25:51)
showHumanReadableDeadlines: true

# Mask Account IDs In Console: Optional. Default: false.
#   Will mask most of any account IDs in the Archon console, if you're screenshot happy, but don't want people knowing your IDs :)
#   Example: ID 12345678901234567890 => 1XXXXXXXXXXXXXXXX890
maskAccountIdsInConsole: false

# Use 24 Hour Time: Optional. Default: false. Shows times in console as 24 hour format.
use24HourTime: false

# Define PoC Chains to mine here, Archon will exit if there are no chains configured, you need at least one.
# Template:
#  - name: BURST - VLP [Pool]               # Friendly name to display in the log for this chain.
#    enabled: true                          # Optional. Default: True.
#    priority: 0                            # Zero-based priority. 0 = highest. Only used if running in Priority mode.
#                                               NOTE: Must be unique, you can't have multiple chains with the same priority!
#    isBhd: false                           # Optional. Default: False. Is this chain for BHD / BTCHD / BitcoinHD?
#    isPool: true                           # Optional. Default: False. Is this chain for pool mining?
#    url: "http://voiplanparty.com:8124"    # The URL to connect to the chain on.
#    historicalRounds: 360                  # Optional. Default: 360. Number of rounds to keep best deadlines for, to use in the upcomping web ui.
#    targetDeadline: 31536000               # Optional. Specify a hard target deadline for this chain (in seconds).
# Passphrases for different IDs go here, if you are solo mining Burst via Archon.
#    numericIdToPassphrase:
#      12345678901234567890: passphrase for this numeric id goes here
# If you wish to have separate target deadlines for each numeric ID for this chain only, you can specify that here
# NOTE: Deadlines specified here will OVERRIDE ANY OTHERS except a max deadline received from upstream.
#    numericIdToTargetDeadline:
#      12345678901234567890: 86400          # 1 day target deadline for ID 12345678901234567890
#    color: cyan                            # Color to use for console logging for this chain.
#                                             Valid colors are: "green", "yellow", "blue", "magenta", "cyan", "white".
#    getMiningInfoInterval: 3               # Optional. Default: 3. Interval (in seconds) to poll for mining info.
#    useDynamicDeadlines: true              # Optional. Default: False. If true, will use your total plots size and current network difficulty to calculate a target deadline.
#    allowLowerBlockHeights: false          # Optional. Default: False. If true, Archon will allow new blocks with a lower height than the previous block, for this chain only.
#    requeueInterruptedBlocks: true         # Optional. Default: True. Only used in priority mode with interruptLowerPriorityBlocks on.
#                                               TRUE: Interrupted blocks will be requeued and started again as soon as possible.
#                                              FALSE: Interrupted blocks will be discarded.
pocChains:
  - name: BTCHD - [HDPool]
    priority: 0
    isBhd: true
    isPool: true
    url: "http://localhost:60100"
    color: cyan
  - name: BURST - VLP [Pool]
    priority: 1
    isPool: true
    url: "http://voiplanparty.com:8124"
    color: magenta
  - name: BURST - TestNet [Pool]
    enabled: false
    priority: 2
    isPool: true
    url: "http://75.100.126.230:8124"
    targetDeadline: 7200
    color: blue"#.to_string();
    }

    /*pub fn try_parse_config(file: File) -> (Option<Self>, bool) {
        match serde_yaml::from_reader(file) {
            Ok(cfg) => (Some(cfg), true),
            Err(parse_err) => {
                println!(
                    "{} {}",
                    "ERROR".red().underline(),
                    "An error was encountered while attempting to parse the config file."
                );
                println!(
                    "   {} {}",
                    "MSG".red().underline(),
                    format!("{}", parse_err).red()
                );
                println!(
                    "  {} {} {}{}",
                    "HELP".green().underline(),
                    "Please check your YAML syntax (perhaps paste it into".green(),
                    "yamllint.com".blue(),
                    ")".green()
                );
                (None, false)
            }
        }
    }*/

    #[logfn(Err = "Error", fmt = "Unable to parse config file: {:?}")]
    pub fn parse_config(file: File) -> Result<(Self), ArchonError> {
        match serde_yaml::from_reader(file) {
            Ok(cfg) => Ok((cfg)),
            Err(_) => {
                Err(ArchonError::new("Please check your YAML syntax (Perhaps paste it into yamllint.com)"))
            }
        }
    }

    // TODO: This will probably have to be fixed / changed. Not quite sure yet.
    #[logfn(Err = "Error", fmt = "Error creating new default config file: {:?}")]
    pub fn query_create_default_config() -> Result<(), ArchonError> {
        println!("\n  Would you like to create a default configuration file?");
        println!("  {}", "WARNING: THIS WILL OVERWRITE AN EXISTING FILE AND CANNOT BE UNDONE!".yellow());
        println!("  {}", "Type \"y\" and <Enter> to create the file, or just hit <Enter> to exit:".cyan());

        let mut resp = String::new();
        match std::io::stdin().read_line(&mut resp) {
            Ok(_) => {
                if resp.trim().to_lowercase() == "y" {
                    let default_config_yaml = crate::Config::create_default();
                    match File::create("archon.yaml") {
                        Ok(mut file) => {
                            use std::io::Write;
                            match file.write_all(&default_config_yaml.as_bytes()) {
                                Ok(_) => {
                                    println!("  {}", "Default config file save to archon.yaml".green());
                                }
                                Err(_) => {}
                            };
                        }
                        Err(err) => {
                            Err(ArchonError::new(err))
                        }
                    };
                }
            }
            Err(_) => {
                Ok(())
            }
        };
    }

    #[logfn(Err = "Error", fmt = "Unable to serialize to yaml: {:?}")]
    pub fn to_yaml(&self) -> Result<String, ArchonError> {
        //serde_yaml::to_string(self).unwrap()
        match serde_yaml::to_string(self) {
            Ok(yaml) => {
                Ok(yaml)
            },
            Err(why) => {
                Err(ArchonError::new(&format!("{:?}", why)))
            },
        }
    }w
}
