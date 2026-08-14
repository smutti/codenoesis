# CodeNoesis experimental local bundle

This digest-named directory is an unsigned, unsupported and not-Verified
engineering artifact for `local-experimental-r17`.

1. Validate `manifest.json` against every payload before execution.
2. Run `bin/noesis` (`bin/noesis.exe` on Windows) with
   `--config etc/codenoesis/config.json config validate --format json`.
3. Run the same binary and configuration with
   `profile --id local-experimental-r17 --format json`.
4. Upgrade by placing a new digest directory beside this one and selecting it
   only after both checks succeed.
5. Roll back by selecting the retained prior digest directory and rerunning
   both checks.
6. Uninstall by stopping processes and removing exactly the selected digest
   directory.

The product does not mutate PATH, shell profiles, home or system configuration,
package-manager state, registry or plist values, services, scheduled jobs, or
a hidden activation pointer.
