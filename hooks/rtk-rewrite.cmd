@echo off
REM rtk-hook-version: 2
REM RTK Claude Code hook — rewrites commands to use rtk for token savings.
REM Windows variant: delegate to the native Rust Claude/Copilot hook processor.

where rtk >nul 2>nul
if errorlevel 1 (
  echo [rtk] WARNING: rtk is not installed or not in PATH. Hook cannot rewrite commands. Install: https://github.com/rtk-ai/rtk#installation 1>&2
  exit /b 0
)

rtk hook copilot
