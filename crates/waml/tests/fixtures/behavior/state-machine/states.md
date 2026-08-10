---
type: uml.StateMachineDiagram
title: Light Cycle
---
# Light Cycle

## Nodes
### initial Start
- transitions to Idle
### Idle
- entry: `enable`
- do: `wait`
- exit: `disable`
- on `press` when `ready` transitions to Active
### Active
- entry: `light`
- do: `hold`
- exit: `dim`
- on `press` when `busy` transitions to Active
- on `timeout` transitions to Idle
- on `power_off` transitions to Done
### final Done
