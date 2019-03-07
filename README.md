
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Discord](https://img.shields.io/static/v1.svg?logo=discord&label=Archon%20Discord&message=https://discord.gg/ZdVbrMn)](https://discord.gg/ZdVbrMn)
# Archon
A collision free, multi-chain proof of capacity mining proxy.

## What Archon does:
- Turns a regular single-chain miner into a multi-chain miner via intelligent and customizable queue management
- Is compatible with Scavenger and Blago proof-of-capacity (PoC) mining software
- Like Scavenger, Archon is written using the Rust systems language; memory-safe, cross-platform, low-footprint and stable

## What Archon (currently) does not do:
- Mine
- Replace HDProxy
- Provide lambos

## How it works
Archon acts as an intermediary between your mining software and your upstream pool/wallet(s).
Trying to be brief, for each chain you instruct Archon to manage, it will:
```
- Create a thread and poll for *new* mining info
  - Determines if the new mining info should be mined immediately, or queued, using a logical flow system
- Wait for connections from your miners, just like a normal pool/wallet would do
  - Upon receiving a `getMiningInfo` request, asynchronously sends the *current mining info* to the miner
  - Upon receiving a `submitNonce` request (deadline submission) from a miner, uses logic to determine which chain the submission is for and whether to forward the deadline to the upstream pool/wallet
    - Do not send Upstream: Asynchronously sends a fake confirmation back to the miner
      - This will be the case if either of the following is true:
        - The submitted deadline is greater than the target deadline for this chain
        - The submitted deadline is greater than previously submitted deadlines by the accound ID for this block height
    - Send Upstream: Asynchronously sends the deadline submission upstream, and awaits the result, forwarding the result back to the miner
- Once a second, in a separate thread, processes any blocks waiting to be mined, using logic to determine when to start mining them.
```

## Defining your mining chains
Archon supports mining multiple chains in either a `priority mode (default)` or a `first in, first out mode`, you would only use the latter if you didn't value mining any one chain over another.

Your PoC chains are defined in the `archon.yaml` configuration file, [see below](https://github.com/Bloodreaver/Archon/new/master?readme=1#sample-configuration-file).

Note: You must have at least one PoC Chain defined, or Archon will have nothing to do!

Example layout, using the bare minimum information required by Archon:
```yaml
pocChains:
  - name: First Chain
    priority: 0
    url: "http://localhost:60100"
    isBhd: true
    isPool: true
    color: cyan
  - name: Second Chain
    priority: 1
    url: "http://voiplanparty.com:8124"
    isPool: true
    color: magenta
```

## All Configuration Options for PoC Chains
If you need more control over your chains, you can add any of these parameters to each chain. There is no set order to these.
- `name`
  - Required (But can be blank)
  - Used for displaying a friendly chain name in the Archon console.
- `enabled`
  - Optional. Default = true
  - If this is set to false, Archon will ignore this chain completely.
- `priority`
  - Required (But only used if `priorityMode` = `true`)
  - A 0-based priority index. 0 = highest priority. MUST BE UNIQUE PER CHAIN.
- `isBhd`
  - Optional. Default = false
  - Set to true if the chain is mining BHD/BTCHD/BitcoinHD.
- `isPool`
  - Optional. Default = false
  - Set to true if the chain is mining via a pool.
- `url`
  - Required.
  - Must be a fully qualified URI including protocol, domain/IP and port, eg: "http://voiplanparty.com:8124"
- `historicalRounds`
  - Optional. Default = 360
  - Not used at the moment, but will be used later for statistics displayed in the Web UI (which is not implemented currently).
- `targetDeadline`
  - Optional. Default = 18446744073709551615 (u64::max) or the pool/wallet's maximum deadline, if given.
  - Set this to the desired maximum deadline. Any deadlines submitted to Archon for this chain which are higher than this value will not be sent upstream.
- `numericIdToPassphrase`
  - Optional.
  - Use this section if this chain is for solo mining BURST.
  - Example format:
```yaml
numericIdToPassphrase:
  12345678901234567890: passphrase for this numeric id goes here
```
- `numericIdToTargetDeadline`
  - Optional.
  - Use this section to specify OVERRIDES for target deadlines. 
  - *Note: If a deadline is submitted from an ID specified here, for this chain, this target deadline WILL be used, provided it is under the maximum deadline reported by the upstream pool/wallet, if given.*
  - Example format:
```yaml
numericIdToTargetDeadline:
  12345678901234567890: 86400          # 1 day target deadline for ID 12345678901234567890
```
- `color`
  - Required.
  - Specify a color for Archon to display info for this chain in.
  - Valid colors:
    - green
    - yellow
    - blue
    - magenta
    - cyan
    - white
- `getMiningInfoInterval`
  - Optional. Default = 3 seconds
  - Specify the interval, in seconds, that Archon will request mining info for this chain. Minimum is 1 second.
- `useDynamicDeadlines`
  - Optional. Default = false
  - If set to true, and you have specified a total plot size in your Archon configuration, Archon will calculate your target deadline dynamically for each block, for this chain only.
- `allowLowerBlockHeights`
  - Optional. Default = false
  - If set to true, Archon will change its new-block-detection method from "block height greater than previous" to "block height not equal to previous" for this chain only, which will consequently allow a lower block height to be mined in the same chain.
    - Use case: Only really useful if this chain is pointing at a multi-chain proxy, or a pool that mines multiple chains. *cough PoCC cough*
- `requeueInterruptedBlocks`
  - Optional. Default = true
  - If you disable this feature, this chain's blocks which get interrupted by a higher priority chain **WILL NOT** be requeued and mined after the higher priority chain finishes.
    - Use case: If this chain is a testnet chain or something you don't really care about mining every block for.
   
## Sample configuration file
Archon will look in the working directory (usually the same location as the executable) for `archon.yaml`.

If the file cannot be loaded or is non-existent, Archon will ask you if you would like to generate one. Be warned: If you agree, Archon will overwrite any existing `archon.yaml` file in the working directory, this is not reversible!

Generated File Contents:
```yaml
---
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

# Define PoC Chains to mine here, Archon will exit if there are no chains configured, you need at least one.
# Template:
#  - name: BURST - VLP [Pool]               # Friendly name to display in the log for this chain.
#    enabled: true                          # Optional. Default: True.
#    priority: 0                            # Zero-based priority. 0 = highest. Only used if running in Priority mode.
#                                               NOTE: Must be unique, you can't have multiple chains with the same priority!
#    isBhd: false                           # Optional. Default: False. Is this chain for BHD / BTCHD / BitcoinHD?
#    isPool: true                           # Optional. Default: False. Is this chain for pool mining?
#    url: "http://voiplanparty.com:8124"    # The URL to connect to the chain on.
#    historicalRounds: 360                  # Optional. Default: 360. Number of rounds to keep best deadlines for, to use in the upcoming web ui.
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
    color: blue
```
