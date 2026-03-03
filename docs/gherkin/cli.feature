Feature: CLI command routing and exit behavior
  As a CLI user
  I want command-line arguments to map correctly and failures to return non-zero exits
  So that shell automation can rely on deterministic behavior

  Rule: command dispatch routes arguments to app commands

    Scenario: CLI-001 ls command is mapped with list flags
      Given ls command options
      When CLI runner executes
      Then app receives AppCommand::Ls with matching options

    Scenario: CLI-002 mkroom alias is mapped to mkdir room command
      Given mkroom command options
      When CLI runner executes
      Then app receives AppCommand::Mkdir with room type and deprecation alias flag

    Scenario: CLI-003 transfer command is mapped with transfer options
      Given transfer command options
      When CLI runner executes
      Then app receives AppCommand::Transfer with matching options

    Scenario: CLI-004 config auth ls is mapped to app config command
      Given config auth ls options
      When CLI runner executes
      Then app receives AppCommand::Config with matching subcommand

  Rule: error semantics produce automation-safe failures

    Scenario: CLI-005 app executor errors are propagated unchanged
      Given app executor returns a typed error
      When CLI runner executes the command
      Then runner returns the same error

    Scenario: CLI-006 outcome-level error messages are converted to command failure
      Given app command returns an outcome containing error messages
      When CLI runner executes the command
      Then runner returns command-failed error for non-zero process exit
