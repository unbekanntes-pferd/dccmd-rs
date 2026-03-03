Feature: Config command behavior
  As a CLI user
  I want config subcommands to normalize account scope and manage local secrets consistently
  So that credential and encryption state remains reliable

  Rule: URL normalization and credential scopes

    Scenario: CFG-001 DRACOON base URL normalization strips scheme and path
      Given a DRACOON URL including scheme and path
      When URL normalization runs
      Then normalized base URL contains only host

    Scenario: CFG-002 removing auth token uses normalized account scope
      Given a DRACOON URL with path fragments
      When config auth rm executes
      Then refresh token removal uses normalized base URL account key

    Scenario: CFG-003 encryption secret lookup and removal use crypto account scope
      Given a DRACOON URL
      When config crypto ls and rm execute
      Then crypto account state is resolved and removed for the normalized URL

  Rule: command dispatch to app layer

    Scenario: CFG-004 CLI dispatch maps config auth ls to app config command
      Given config auth ls arguments
      When CLI runner dispatches
      Then app receives matching config command

    Scenario: CFG-005 system-info command failures are propagated to caller
      Given config system-info call returning API error
      When command executes
      Then caller receives the original error
