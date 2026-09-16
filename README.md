# agent-worktree

Run Codex or Claude Code in a Git worktree with a dedicated development
container and isolated Docker Compose resources.

Each named lane owns:

- `.agents/worktrees/<name>` on branch `agent/<name>`
- a development container for that worktree
- a stable Compose project name
- project-scoped networks and named volumes

Docker images and Git objects remain shared for speed. Database, Redis, queue,
and other Compose state remain private to the lane unless the project explicitly
opts into sharing them.

## Requirements

- Git 2.46+ (for `git worktree add --relative-paths` support)
- Docker
- the [Dev Container CLI](https://github.com/devcontainers/cli)
- a `.devcontainer/devcontainer.json` or `.devcontainer.json` in the project
- `codex` or `claude` installed inside the development container

Run `agent-worktree setup` to check your Git version and install the Dev
Container CLI if it's missing:

```sh
agent-worktree setup
```

The project devcontainer should start its databases and other supporting
services. Lifecycle commands such as `postCreateCommand` should install project
dependencies and initialize the lane's isolated database.

## Install

```sh
cargo install --path .
```

This installs the `agent-worktree`, `codex-worktree`, and `claude-worktree`
binaries into `~/.cargo/bin`.

## Run an agent

```sh
agent-worktree --name fix-payments --agent codex -- "Fix the payment migration"
agent-worktree --name review-api --agent claude
```

The agent-specific aliases select the agent automatically:

```sh
codex-worktree --name fix-payments -- exec --full-auto "Fix the migration"
claude-worktree --name fix-payments
```

Named lanes are reusable. Running the same command again resumes the same
worktree, container, and Compose volumes. A process-aware lane lock prevents
two wrappers from launching writing agents in the same lane concurrently and
automatically recovers locks left by dead processes.

## Manage lanes

```sh
agent-worktree list
agent-worktree list --json
agent-worktree doctor fix-payments
agent-worktree stop fix-payments
agent-worktree destroy fix-payments --yes
```

`stop` preserves the worktree and data. `destroy` refuses dirty worktrees,
stops or removes only containers carrying the lane's labels, removes its
Compose-labelled volumes and networks, and removes the worktree. It deliberately
retains `agent/<name>`, so committed work is not deleted and the lane can be
recreated later.

## Isolation contract

The wrapper exports a unique `COMPOSE_PROJECT_NAME` for every repository/lane
pair. Standard Compose container, network, and volume names are therefore
scoped to that lane.

Some Compose options bypass project scoping. Run `doctor` to detect common
problems:

- fixed `container_name` values
- fixed or external volumes and networks
- host networking
- fixed published host ports
- Docker socket mounts
- custom `workspaceMount` configurations that interfere with Git worktrees

Prefer communication over the private Compose network (`db:5432`) instead of
publishing databases on host ports. Use dynamic host ports when a service must
be reachable from the host.

## Git worktrees inside devcontainers

Worktrees are created with relative Git metadata and the wrapper passes
`--mount-git-worktree-common-dir true` to the Dev Container CLI. This lets Git
inside the container reach the repository metadata shared by the worktrees.

Current versions of the Dev Container CLI may not add that mount when the
project defines a custom `workspaceMount`. `agent-worktree doctor` reports this
case; remove the custom mount or provide the Git common-directory mount in the
project configuration before launching agents.

## Security boundary

This tool prevents accidental state collisions. It is not a sandbox for
hostile code: Git worktrees still share repository metadata, and bind mounts or
Docker socket access can expose host resources. Do not mount a host home
directory or Docker socket into agent containers unless that access is
intentional.
