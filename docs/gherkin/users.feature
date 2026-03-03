Feature: Users command behavior
  As an administrator
  I want users commands to enforce selection rules and process user mutations safely
  So that account lifecycle and auth-policy operations remain correct

  Rule: user listing and selection

    Scenario: US-001 listing users with all-pages mode fetches subsequent pages
      Given users list total above one page
      When I run users ls with all mode
      Then additional pages are fetched and merged

    Scenario: US-002 deleting by user name resolves to user id before deletion
      Given an existing user name
      When I run users rm with user name
      Then user id is resolved and deleted

    Scenario: US-003 deleting requires user selector
      Given no user name and no user id
      When I run users rm
      Then the command fails with invalid argument

  Rule: invite and import workflows

    Scenario: US-004 invite resolves room id and delegates API call
      Given an invite target path
      When I run users invite
      Then a room id is resolved and invite call is executed

    Scenario: US-005 CSV import reports success and failure counts
      Given a mixed import batch
      When I run users import
      Then imported, failed, and total counters are reported correctly

  Rule: auth-method and MFA administration

    Scenario: US-006 switch-auth updates only users matching configured selector
      Given a matching auth-method cohort
      When I run users switch-auth
      Then only matching users are updated

    Scenario: US-007 enforce-mfa applies group and auth filters
      Given enforce-mfa options with group and auth method selectors
      When I run users enforce-mfa
      Then only matching users are updated and counted as success

    Scenario: US-008 enforce-mfa requires at least one selector
      Given no auth method, filter, or group selector
      When I run users enforce-mfa
      Then the command fails with invalid argument
