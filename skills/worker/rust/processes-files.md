---
extends: worker/rust/processes-files
---
26. File names with version dots (`kompanion-runner-0.3.1-x86_64-linux-musl`): never use
    `with_extension`, which cuts at the last dot. Build the name: `dir.join(format!("{name}.sha256"))`.
37. (2026-10-04) `tools/qwen/pipeline.py` finds OVMS by the container's current address; a fixed IP
    broke after a reboot (every OVMS job got a 404). Never hard-code a container IP.
