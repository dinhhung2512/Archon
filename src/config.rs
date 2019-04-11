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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_hpool: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_hdpool: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub miner_name: Option<String>,

    #[serde(default)]
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_old_log_files_to_keep: Option<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging_level: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency_logging_level: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_miner_addresses: Option<bool>,
}

impl Config {
    pub fn create_default() -> String {
        return r#"---
###############################################################################################
##                             ARCHON DEFAULT CONFIGURATION FILE                             ##
###############################################################################################
## For help and guidance, see the github readme at https://github.com/Bloodreaver/Archon     ##
## You can also join the Discord at https://discord.gg/ZdVbrMn if you need further help! :)  ##
###############################################################################################

# Grace Period: How long (in seconds) Archon will let blocks mine for.
# NOTE: This value is extremely important, it is used as a timer by Archon to determine how much time must elapse after a block starts
#   before Archon can send the next queued block to be mined. Set it too small, and Archon will instruct your miners to start mining a 
#   new block before they've finished scanning the previous one. Conversely, set it too long, and you risk missing blocks entirely.
#   Ideally it should be set around 5 seconds longer than your regular scan times, 5 seconds just to give it a safety net.
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
# NOTE: This probably isn't your machine's LAN IP, you'll need to change it!
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

# Num Old Log Files to Keep: Optional. Default: 5.
#  Only used if loggingLevel is not <Off>.
numOldLogFilesToKeep: 5

# Logging Level: Optional. Default: Info. Case insensitive.
#   Valid options: off|trace|debug|info|warn|error
loggingLevel: info

# Show Miner Addresses: Optional. Default: false.
#   Shows the IP Address of miner's which submit deadlines.
showMinerAddresses: false

######################################################################################################################
# Define PoC Chains to mine here, Archon will exit if there are no chains configured/enabled, you need at least one! #
######################################################################################################################

# What follows is a default chain configuration, set up to mine BHD via HDProxy running on the same machine as Archon, and
# Burst via the VoipLanParty.com pool.
# A Testnet pool chain is also there, but disabled by default as most people won't wish to mine it.
######## https://github.com/Bloodreaver/Archon#defining-your-mining-chains ########

pocChains:
### BHD via HDPool - no need for HDProxy ###
  - name: BTCHD - [HDPool]
    priority: 0
    isHdpool: true
    accountKey: abcdefg-abcdefg-abcdefg-abcdefg
    url: "" # Not required for HDPool, Archon knows it. If you wish to use HDProxy you can specify a URL here.
    color: cyan

### BURST via VLP pool (http://voiplanparty.com) ###
  - name: BURST - VLP [Pool]
    priority: 1
    isPool: true
    url: "http://voiplanparty.com:8124"
    color: magenta

### BURST Testnet Pool - Disabled by default ###
  - name: BURST - TestNet [Pool]
    enabled: false
    priority: 2
    isPool: true
    url: "http://75.100.126.230:8124"
    targetDeadline: 7200
    color: blue"#.to_string();
    }

    pub fn parse_config(file: File) -> Result<Self, ArchonError> {
        match serde_yaml::from_reader(file) {
            Ok(cfg) => Ok(cfg),
            Err(why) => {
                Err(ArchonError::new(&format!("{} {}\n  {} {}\n  {} {} {}{}", 
                    "ERROR".red().underline(),
                    "An error was encountered while attempting to parse the config file.",
                    "MSG".red().underline(),
                    format!("{}", why).red(),
                    "HELP".green().underline(),
                    "Please check your YAML syntax (Perhaps paste it into".green(),
                    "yamlline.com".blue(),
                    ")".green())))
            }
        }
    }

    pub fn to_yaml(&self) -> Result<String, ArchonError> {
        match serde_yaml::to_string(self) {
            Ok(yaml) => Ok(yaml),
            Err(why) => Err(ArchonError::new(&format!("{:?}", why))),
        }
    }
}
