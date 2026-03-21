# RTK - Copilot Global Instructions

**Usage**: Token-optimized CLI proxy (60-90% savings on dev operations)

This file is intended for `~/.copilot/copilot-instructions.md` so GitHub Copilot CLI and VS Code Copilot Chat can load RTK guidance globally.

## Golden Rule

Always prefer `rtk` for shell commands that produce verbose output.

Examples:

```bash
rtk git status
rtk git diff
rtk cargo test
rtk npm run build
rtk pytest -q
rtk docker ps
```

## Meta Commands

```bash
rtk gain              # Show token savings analytics
rtk gain --history    # Show command usage history with savings
rtk discover          # Analyze sessions for missed RTK usage
rtk proxy <cmd>       # Run raw command without filtering
```

## Verification

```bash
rtk --version
rtk gain
where rtk            # Windows
which rtk            # macOS/Linux
```

⚠️ **Name collision**: If `rtk gain` fails, you may have the wrong `rtk` installed.
