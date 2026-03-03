Feature: Upload command behavior
  As a CLI user
  I want upload workflows to be predictable under normal and high-concurrency workloads
  So that file and container uploads are safe and performant

  Rule: upload validates source and target constraints

    Scenario: UP-001 single file upload succeeds for a valid node target
      Given a local file source and valid DRACOON container target
      When I run upload
      Then the file is uploaded to the target container

    Scenario: UP-002 container upload requires recursive flag
      Given a local directory source and valid DRACOON target
      When I run upload without recursive mode
      Then the command fails with invalid argument

    Scenario: UP-003 public upload share only accepts file sources
      Given a public upload share target and a directory source
      When I run upload
      Then the command fails with invalid path

    Scenario: UP-004 recursive traversal discovers nested files and folders
      Given a nested local folder tree
      When recursive upload traversal is executed
      Then all files and folders are discovered and normalized

  Rule: upload concurrency remains bounded by velocity controls

    Scenario: UP-CONC-001 effective velocity is clamped to allowed bounds
      Given out-of-range velocity values
      When effective velocity is resolved
      Then the value is clamped between minimum and maximum velocity

    Scenario: UP-CONC-002 concurrent request budget scales with velocity multiplier
      Given valid velocity values
      When concurrent request budget is computed
      Then concurrency equals velocity multiplied by the configured multiplier

    Scenario: UP-CONC-003 high-concurrency execution never exceeds computed cap
      Given many upload tasks and high velocity
      When tasks run concurrently
      Then peak in-flight tasks never exceed the configured concurrency cap

    Scenario: UP-CONC-004 low-velocity execution limits in-flight tasks
      Given many upload tasks and low velocity
      When tasks run concurrently
      Then peak in-flight tasks stay within the low-velocity cap
