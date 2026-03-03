Feature: Reports command behavior
  As an administrator
  I want reports commands to enforce compatibility and filtering semantics
  So that audit output is complete and accurate

  Rule: compatibility and event retrieval

    Scenario: RP-001 permissions report rejects unsupported DRACOON API versions
      Given DRACOON API version 5.x
      When permissions report is requested
      Then command fails with compatibility error

    Scenario: RP-002 events listing with all mode fetches additional pages
      Given events total above one page
      When I run reports events with all mode
      Then additional pages are fetched and merged

    Scenario: RP-003 event options parse date and status filters correctly
      Given start date, end date, and status options
      When event options are parsed
      Then normalized option values are available for query building

  Rule: permissions collection modes

    Scenario: RP-004 permissions with explicit user filter avoids user listing
      Given a user filter in permissions options
      When permissions are requested
      Then node permissions are fetched with that filter only

    Scenario: RP-005 permissions without filter enumerates users and aggregates results
      Given no permissions filter
      When permissions are requested
      Then all users are paginated and queried individually

  Rule: operation type metadata

    Scenario: RP-006 operation type retrieval delegates to API
      Given reports operation-types command
      When command executes
      Then operation type list is returned from API
