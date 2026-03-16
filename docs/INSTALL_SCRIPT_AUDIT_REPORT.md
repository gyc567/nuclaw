# NuClaw Installation Scripts Audit Report

## 1. Executive Summary
An audit of the one-click installation scripts (`install.sh` and `install.ps1`) was conducted to evaluate their simplicity, user guidance, and overall user experience (UX). 

**Overall Verdict:** 
The scripts are highly automated and robust, successfully handling cross-platform detection, binary downloading, and environment scaffolding. However, they currently function as **"silent installers"** rather than **"guided installers"**. To fully meet the goals of step-by-step guidance and exceptional UX, the scripts need to transition from zero-interaction to an interactive, user-consented flow.

## 2. Detailed Assessment against Goals

### Goal 1: Simple to Use (Rating: Excellent)
*   **Pros:** 
    *   Single-command execution (`curl | bash` or running the `.ps1`).
    *   Automatically detects OS (Linux, macOS, Windows) and CPU architecture (x86_64, arm64).
    *   Automatically creates the required directory structures (`store`, `data`, `logs`, etc.) and default configuration files.
*   **Cons:** 
    *   The binary is installed to `~/.nuclaw/nuclaw`. This path is not in the system's `$PATH` by default, meaning users must type the full path to run the tool.

### Goal 2: Step-by-Step Guidance (Rating: Needs Improvement)
*   **Pros:**
    *   Excellent use of colored terminal output (`[STEP]`, `[INFO]`, `[OK]`) to narrate what the script is doing in the background.
    *   Provides a helpful "Quick Start" summary at the end.
*   **Cons:**
    *   There is **no interactive guidance** during the installation. The user is just a spectator.
    *   It leaves the user at the terminal prompt at the end, telling them to manually copy `.env.example`, edit it, and then manually run `--onboard`. A truly guided experience would chain these steps together.

### Goal 3: Good User Experience (Rating: Fair)
*   **Pros:**
    *   Graceful fallback: If a pre-built binary isn't found for the architecture, it attempts to build from source.
*   **Cons (Critical UX Issues):**
    *   **Aggressive Source Build:** If the binary download fails, the script *silently* installs Rust (`rustup`) and begins compiling the project from source. Rust compilation can take 5-20 minutes and consume massive CPU/RAM. Doing this without explicit user consent is a poor experience.
    *   **Aggressive Service Creation:** The script automatically attempts to install systemd services, macOS launchd agents, and Windows Scheduled Tasks. Creating background daemons without asking the user is invasive and can trigger security warnings (especially the Windows Scheduled Task).

---

## 3. Recommended Optimization Plan (Actionable Steps)

To elevate the installation process to a top-tier developer experience, I recommend the following enhancements:

### Recommendation 1: Introduce Interactive Prompts
Add a prompt mechanism (with a `-y` or `--quiet` flag for CI/CD environments) to ask for user consent on major actions.
*   *Prompt:* "No pre-built binary found. Do you want to install Rust and build from source? (This may take several minutes) [Y/n]"
*   *Prompt:* "Would you like to install NuClaw as a background service? [y/N]"

### Recommendation 2: PATH Configuration
Ask the user if they want to add NuClaw to their system PATH or create a symlink.
*   *Linux/macOS:* Offer to symlink `~/.nuclaw/nuclaw` to `/usr/local/bin/nuclaw` or append `export PATH=$PATH:~/.nuclaw` to `.bashrc`/`.zshrc`.
*   *Windows:* Offer to add `$InstallPath` to the User Environment `PATH` variable.

### Recommendation 3: Chain the Onboarding Process
Instead of just printing instructions at the end, the script should ask if the user is ready to configure the system now.
*   *Prompt:* "Installation complete! Would you like to launch the setup wizard now? [Y/n]"
*   If yes, the script automatically executes `~/.nuclaw/nuclaw --onboard`.

## 4. Conclusion
The current scripts provide a very strong, automated foundation. By injecting a few strategic, interactive prompts, we can transform the script from a "blind executor" into a friendly, step-by-step setup wizard that drastically lowers the barrier to entry for new users.
