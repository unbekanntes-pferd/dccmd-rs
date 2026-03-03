Feature: Groups command behavior
  As an administrator
  I want group and group-user operations to enforce selectors and mapping rules
  So that membership administration remains correct

  Rule: group lifecycle operations

    Scenario: GR-001 list groups with all mode fetches additional pages
      Given group list total above one page
      When I run groups ls with all mode
      Then all pages are fetched and merged

    Scenario: GR-002 creating a group delegates to API
      Given a new group name
      When I run groups create
      Then group creation request is sent

    Scenario: GR-003 deleting a group works by id or by name resolution
      Given either group id or group name
      When I run groups rm
      Then the resolved group id is deleted

    Scenario: GR-004 deleting a group requires id or name
      Given no group id and no group name
      When I run groups rm
      Then the command fails with invalid argument

  Rule: group user administration

    Scenario: GR-005 group users listing resolves selected group and lists members
      Given a group name selector
      When I run groups users ls
      Then users for the selected group are listed

    Scenario: GR-006 adding a group user resolves user and group selectors
      Given group and user names
      When I run groups users add
      Then both selectors are resolved and membership is created

    Scenario: GR-007 adding group user requires user selector
      Given no user name and no user id
      When I run groups users add
      Then the command fails with invalid argument
