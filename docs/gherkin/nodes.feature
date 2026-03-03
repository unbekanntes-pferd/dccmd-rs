Feature: Nodes command behavior
  As a CLI user
  I want nodes commands to behave consistently across app/service/api layers
  So that listing, copying, deleting, creating, and downloading nodes are predictable and testable

  Rule: ls supports direct path listing, wildcard search, and pagination

    Scenario: LS-001 list non-search path
      Given a valid node path without wildcard
      When I run ls on that path
      Then nodes are fetched from the parent path
      And an info outcome reports listed node count and path

    Scenario: LS-002 list wildcard search
      Given a wildcard search path
      When I run ls on that path
      Then search is used instead of direct node listing

    Scenario: LS-003 list all pages
      Given a listing result with total greater than one page
      When I run ls with all-pages enabled
      Then additional pages are fetched and merged

  Rule: cp handles single source and wildcard source robustly

    Scenario: CP-001 copy single source node
      Given a source node path and valid target path
      When I run cp
      Then exactly one source node id is copied to the target parent

    Scenario: CP-002 copy wildcard source
      Given a wildcard source path and valid target path
      When I run cp
      Then all matching source file ids are copied to the target parent

    Scenario: CP-003 copy fails for invalid source
      Given an invalid source node path
      When I run cp
      Then the command returns an invalid source path error

    Scenario: CP-004 copy fails for invalid target
      Given a valid source node path and invalid target path
      When I run cp
      Then the command returns an invalid target path error

  Rule: rm protects against unsafe deletes and confirms destructive actions

    Scenario: RM-001 reject wildcard delete without recursive
      Given a wildcard delete path
      When I run rm without recursive
      Then deletion is rejected with a recursive requirement error

    Scenario: RM-002 reject non-empty container delete without recursive
      Given a folder or room node path
      When I run rm without recursive
      Then deletion is rejected with a recursive requirement error

    Scenario: RM-003 confirm wildcard batch delete
      Given a wildcard delete path and recursive enabled
      When I confirm the delete prompt
      Then all prepared non-room node ids are deleted

    Scenario: RM-004 cancel wildcard batch delete
      Given a wildcard delete path and recursive enabled
      When I decline the delete prompt
      Then no delete call is executed

    Scenario: RM-005 confirm room delete
      Given a room node path and recursive enabled
      When I confirm room deletion
      Then the room node is deleted

    Scenario: RM-006 cancel room delete
      Given a room node path and recursive enabled
      When I decline room deletion
      Then no delete call is executed
      And an explicit cancellation error is emitted

  Rule: mkdir defaults to folder behavior and enforces room constraints

    Scenario: MKDIR-001 default folder creation
      Given mkdir without explicit type
      When I run mkdir
      Then a folder is created
      And no deprecation warning is emitted

    Scenario: MKDIR-002 mkroom alias emits deprecation warning
      Given mkroom command usage
      When I run mkroom
      Then a deprecation warning is emitted
      And room creation still succeeds

    Scenario: MKDIR-003 folder rejects room-only options
      Given folder creation with room-only options
      When I run mkdir
      Then the command fails with invalid argument

    Scenario: MKDIR-004 room inherit defaults to true when no admin users are provided
      Given room creation without admin users and inherit false flag
      When I run mkdir --type room
      Then inherit permissions is treated as true

    Scenario: MKDIR-005 room inherit follows flag when admin users are provided
      Given room creation with explicit admin users and inherit false flag
      When I run mkdir --type room
      Then inherit permissions is false

  Rule: download supports search/file/container flows and cross-style local paths

    Scenario: DL-001 app delegates download command to platform and writes success summary
      Given a download command
      When app executes the command
      Then platform download is called with source and target
      And a success outcome reports downloaded item count and target root

    Scenario: DL-002 search query file discovery filters non-file results
      Given a search query result with files and non-files
      When search query files are resolved
      Then only files are returned

    Scenario: DL-003 search query file discovery fetches additional pages
      Given search query total above one page
      When search query files are resolved
      Then additional pages are fetched and merged

    Scenario: DL-004 container discovery excludes rooms when include_rooms is false
      Given container traversal without include_rooms
      When sub containers are loaded
      Then only folders are returned

    Scenario: DL-005 container download target planning supports unix and windows path styles
      Given equivalent container trees
      When local targets are planned using unix and windows mock filesystems
      Then normalized local directory and file targets are correct for both styles

    Scenario: DL-006 public share download rejects recursive mode
      Given a public download share source path
      When I run download with recursive enabled
      Then the command fails with an invalid argument error

    Scenario: DL-007 public share source path must contain access key
      Given a public download share source path ending with a trailing slash
      When the access key is resolved
      Then resolving fails with invalid path

    Scenario: DL-008 multi-file download fails when one target mapping is missing
      Given a file list where one node id has no target path mapping
      When I run multi-file download
      Then the command fails with invalid path for the missing node id

    Scenario: DL-009 multi-file download succeeds when all targets are mapped
      Given a file list where all node ids have valid target path mappings
      When I run multi-file download
      Then all target files are written

    Scenario: DL-010 multi-file download propagates per-file API errors
      Given a file list where one file download returns an API error
      When I run multi-file download
      Then the command completes with aggregated partial failures
      And failed item details are included in download outcome data

    Scenario: DL-011 multi-file download aggregates all failed transfers
      Given a file list where every file download returns an API error
      When I run multi-file download
      Then the command completes with zero successes and failed count equal to requested count

    Scenario: DL-012 sub-room filtering paginates beyond first page
      Given a container with more than one page of sub-rooms
      When files in sub-rooms are filtered
      Then all sub-room pages are fetched before filtering

  Rule: download concurrency respects bounded parallelism from velocity

    Scenario: DL-CONC-001 effective velocity is clamped to allowed bounds
      Given out-of-range download velocity values
      When effective velocity is resolved
      Then the value is clamped between minimum and maximum velocity

    Scenario: DL-CONC-002 high velocity runs with bounded in-flight downloads
      Given many files and high velocity
      When multi-file download runs concurrently
      Then peak in-flight downloads do not exceed the configured concurrency cap

    Scenario: DL-CONC-003 low velocity limits in-flight downloads
      Given many files and low velocity
      When multi-file download runs concurrently
      Then peak in-flight downloads remain within the low-velocity cap
