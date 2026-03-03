Feature: Transfer command behavior
  As a CLI user
  I want transfer to reliably stream content between DRACOON targets
  So that cross-instance copy operations are safe and observable

  Rule: transfer command dispatch and platform delegation

    Scenario: TR-001 CLI dispatch maps transfer arguments to app command options
      Given a transfer command with overwrite, share, and classification flags
      When CLI dispatch executes the command
      Then app receives a transfer command with matching options

    Scenario: TR-002 app delegates transfer execution to the platform
      Given a transfer app command
      When app executes the command
      Then platform transfer is invoked with source, target, and options

    Scenario: TR-003 transfer platform errors propagate to the caller
      Given a transfer app command that fails in platform
      When app executes the command
      Then the error is propagated without masking

  Rule: transfer validates encrypted and sharing constraints

    Scenario: TR-004 sharing to encrypted targets is rejected
      Given a transfer command with share enabled and encrypted target
      When transfer executes
      Then the command fails with invalid argument

    Scenario: TR-005 source and target nodes must resolve from provided paths
      Given invalid source or target transfer paths
      When transfer resolves nodes
      Then transfer fails with invalid path
