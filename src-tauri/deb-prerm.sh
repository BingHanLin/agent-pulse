#!/bin/bash
# Pre-removal script for AgentPulse deb package
# Removes all agent integrations (hook scripts, settings entries, plugins)

/usr/bin/agent-pulse --cleanup 2>/dev/null || true
