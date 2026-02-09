# Command Center - Design Ideations

> All designs for 13" Mac terminal (~100×35)

---

## Logo: Pixel Style (2 lines)

```
▄▄ ▄▄ ▄▄ ▄▄
▀  █▀ █▄ █▄▀
```

---

# Ideation 1: Spacious Layout

> Apple-inspired, fits 5-6 threads comfortably

## Home

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 45%   2.1g                                                    ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     2 need you                                                                                 ║
║                                                                                                ║
║     Auth Refactor                                                                   waiting    ║
║     plan ready · ~/projects/api                                                                ║
║                                                                                                ║
║     Settings Page                                                                   question   ║
║     "which auth provider should I use?" · ~/projects/tui                                       ║
║                                                                                                ║
║                                                                                                ║
║     2 working                                                                                  ║
║                                                                                                ║
║     API Endpoints                                        ●●●●○○○                       12m     ║
║     ~/projects/api                                                                             ║
║                                                                                                ║
║     Test Suite                                           ●●○○○○○                        3m     ║
║     ~/projects/tui                                                                             ║
║                                                                                                ║
║                                                                                                ║
║     1 ready to test                                                                            ║
║                                                                                                ║
║     DB Migration                                                                    verify     ║
║     ~/projects/db                                                                              ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  scroll · click open · ⇥⇥ recent · /threads search all                                         ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

---

# Ideation 2: Compact Layout

> Maximum density, fits 13-15 threads

## Home

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 78%   4.2g   12 agents                                        ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     3 need you                                                                                 ║
║                                                                                                ║
║     Auth Refactor          ~/api       plan      "plan ready for review"            waiting    ║
║     Settings Page          ~/tui       normal    "which auth provider?"             question   ║
║     Payment Flow           ~/pay       plan      "plan ready for review"            waiting    ║
║                                                                                                ║
║                                                                                                ║
║     6 working                                                                                  ║
║                                                                                                ║
║     API Endpoints          ~/api       exec      ●●●●○○○  4/7                           12m    ║
║     Test Suite             ~/tui       exec      ●●○○○○○  2/5                            3m    ║
║     Docs Generator         ~/docs      exec      ●●●●●○○  5/7                            8m    ║
║     Search Index           ~/search    exec      ●●●○○○○  3/6                            5m    ║
║     Lint Fixes             ~/lib       exec      ●○○○○○○  1/6                            1m    ║
║     DB Optimize            ~/db        exec      ●●●●○○○  4/7                            6m    ║
║                                                                                                ║
║                                                                                                ║
║     4 ready to test                                                                            ║
║                                                                                                ║
║     DB Migration           ~/db        done      completed                  2h ago      verify ║
║     Cache Layer            ~/api       done      completed                  4h ago      verify ║
║     User Auth v2           ~/auth      done      completed                  1d ago      verify ║
║     Rate Limiter           ~/api       done      completed                  3h ago      verify ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  scroll · click open · ⇥⇥ recent · /threads search all                                         ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

---

# Ideation 3: With Hover Actions

> Action buttons appear on hover/selection

## 3A: Spacious with Hover Actions

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 45%   2.1g                                                    ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     2 need you                                                                                 ║
║                                                                                                ║
║  ┃  Auth Refactor                                                                   waiting    ║
║  ┃  plan ready · ~/projects/api                                 [approve] [reject] [view]      ║
║                                                                                                ║
║     Settings Page                                                                   question   ║
║     "which auth provider should I use?" · ~/projects/tui                                       ║
║                                                                                                ║
║                                                                                                ║
║     2 working                                                                                  ║
║                                                                                                ║
║     API Endpoints                                        ●●●●○○○                       12m     ║
║     ~/projects/api                                                                             ║
║                                                                                                ║
║     Test Suite                                           ●●○○○○○                        3m     ║
║     ~/projects/tui                                                                             ║
║                                                                                                ║
║                                                                                                ║
║     1 ready to test                                                                            ║
║                                                                                                ║
║     DB Migration                                                                    verify     ║
║     ~/projects/db                                                                              ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  scroll · click open · ⇥⇥ recent · /threads search all                                         ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## 3B: Spacious - Hover on Working Thread

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 45%   2.1g                                                    ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     2 need you                                                                                 ║
║                                                                                                ║
║     Auth Refactor                                                                   waiting    ║
║     plan ready · ~/projects/api                                                                ║
║                                                                                                ║
║     Settings Page                                                                   question   ║
║     "which auth provider should I use?" · ~/projects/tui                                       ║
║                                                                                                ║
║                                                                                                ║
║     2 working                                                                                  ║
║                                                                                                ║
║  ┃  API Endpoints                                        ●●●●○○○                       12m     ║
║  ┃  ~/projects/api                                                          [stop] [view]      ║
║                                                                                                ║
║     Test Suite                                           ●●○○○○○                        3m     ║
║     ~/projects/tui                                                                             ║
║                                                                                                ║
║                                                                                                ║
║     1 ready to test                                                                            ║
║                                                                                                ║
║     DB Migration                                                                    verify     ║
║     ~/projects/db                                                                              ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  scroll · click open · ⇥⇥ recent · /threads search all                                         ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## 3C: Spacious - Hover on Ready to Test

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 45%   2.1g                                                    ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     2 need you                                                                                 ║
║                                                                                                ║
║     Auth Refactor                                                                   waiting    ║
║     plan ready · ~/projects/api                                                                ║
║                                                                                                ║
║     Settings Page                                                                   question   ║
║     "which auth provider should I use?" · ~/projects/tui                                       ║
║                                                                                                ║
║                                                                                                ║
║     2 working                                                                                  ║
║                                                                                                ║
║     API Endpoints                                        ●●●●○○○                       12m     ║
║     ~/projects/api                                                                             ║
║                                                                                                ║
║     Test Suite                                           ●●○○○○○                        3m     ║
║     ~/projects/tui                                                                             ║
║                                                                                                ║
║                                                                                                ║
║     1 ready to test                                                                            ║
║                                                                                                ║
║  ┃  DB Migration                                                                    verify     ║
║  ┃  ~/projects/db                                               [verify] [issue] [archive]     ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  scroll · click open · ⇥⇥ recent · /threads search all                                         ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## 3D: Compact with Hover Actions

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 78%   4.2g   12 agents                                        ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     3 need you                                                                                 ║
║                                                                                                ║
║  ┃  Auth Refactor    ~/api    plan   "plan ready"    waiting   [approve] [reject] [view]       ║
║     Settings Page    ~/tui    normal "which auth?"   question                                  ║
║     Payment Flow     ~/pay    plan   "plan ready"    waiting                                   ║
║                                                                                                ║
║                                                                                                ║
║     6 working                                                                                  ║
║                                                                                                ║
║     API Endpoints          ~/api       exec      ●●●●○○○  4/7                           12m    ║
║     Test Suite             ~/tui       exec      ●●○○○○○  2/5                            3m    ║
║     Docs Generator         ~/docs      exec      ●●●●●○○  5/7                            8m    ║
║     Search Index           ~/search    exec      ●●●○○○○  3/6                            5m    ║
║     Lint Fixes             ~/lib       exec      ●○○○○○○  1/6                            1m    ║
║     DB Optimize            ~/db        exec      ●●●●○○○  4/7                            6m    ║
║                                                                                                ║
║                                                                                                ║
║     4 ready to test                                                                            ║
║                                                                                                ║
║     DB Migration           ~/db        done      completed                  2h ago      verify ║
║     Cache Layer            ~/api       done      completed                  4h ago      verify ║
║     User Auth v2           ~/auth      done      completed                  1d ago      verify ║
║     Rate Limiter           ~/api       done      completed                  3h ago      verify ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  scroll · click open · ⇥⇥ recent · /threads search all                                         ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## 3E: Compact - Hover on Working Thread

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 78%   4.2g   12 agents                                        ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     3 need you                                                                                 ║
║                                                                                                ║
║     Auth Refactor    ~/api    plan   "plan ready"    waiting                                   ║
║     Settings Page    ~/tui    normal "which auth?"   question                                  ║
║     Payment Flow     ~/pay    plan   "plan ready"    waiting                                   ║
║                                                                                                ║
║                                                                                                ║
║     6 working                                                                                  ║
║                                                                                                ║
║     API Endpoints          ~/api       exec      ●●●●○○○  4/7                           12m    ║
║  ┃  Test Suite             ~/tui       exec      ●●○○○○○  2/5        3m      [stop] [view]     ║
║     Docs Generator         ~/docs      exec      ●●●●●○○  5/7                            8m    ║
║     Search Index           ~/search    exec      ●●●○○○○  3/6                            5m    ║
║     Lint Fixes             ~/lib       exec      ●○○○○○○  1/6                            1m    ║
║     DB Optimize            ~/db        exec      ●●●●○○○  4/7                            6m    ║
║                                                                                                ║
║                                                                                                ║
║     4 ready to test                                                                            ║
║                                                                                                ║
║     DB Migration           ~/db        done      completed                  2h ago      verify ║
║     Cache Layer            ~/api       done      completed                  4h ago      verify ║
║     User Auth v2           ~/auth      done      completed                  1d ago      verify ║
║     Rate Limiter           ~/api       done      completed                  3h ago      verify ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  scroll · click open · ⇥⇥ recent · /threads search all                                         ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

---

## Hover Action Buttons Reference

| Thread State | Hover Actions |
|--------------|---------------|
| **need you (waiting)** | `[approve]` `[reject]` `[view]` |
| **need you (question)** | `[answer]` `[view]` |
| **working** | `[stop]` `[view]` |
| **ready to test** | `[verify]` `[issue]` `[archive]` |

---

# Ideation 4: Aggregate Dashboard (Recommended)

> Situational awareness for 50+ threads without listing all

**Concept:** Don't show every thread. Show aggregate status + only expand "need you" with details. Working threads accessible via `/threads`.

## 4A: Dashboard View

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 78%   4.2g                                                    ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     ═══════════════════════════════════════════════════════════════════════════════════════    ║
║                                                                                                ║
║        47 threads          32 agents          12 repos                                         ║
║                                                                                                ║
║     ═══════════════════════════════════════════════════════════════════════════════════════    ║
║                                                                                                ║
║                                                                                                ║
║     ████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░     ║
║     working 24              ready to test 8                     idle 15                        ║
║                                                                                                ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     3 need you                                                                                 ║
║                                                                                                ║
║     Auth Refactor                                                                   waiting    ║
║     plan ready · ~/projects/api                                 [approve] [reject] [view]      ║
║                                                                                                ║
║     Settings Page                                                                   question   ║
║     "which auth provider should I use?" · ~/tui                         [answer] [view]        ║
║                                                                                                ║
║     Payment Flow                                                                    waiting    ║
║     plan ready · ~/projects/pay                                 [approve] [reject] [view]      ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  /threads see all · /repos see repositories · ⇥⇥ recent                                        ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## 4B: Dashboard with Repo Breakdown

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 78%   4.2g                                                    ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║        47 threads     32 agents     across 12 repositories                                     ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     ~/api        ████████░░  8 threads   12 agents   2 need you                                ║
║     ~/tui        ██████░░░░  6 threads    8 agents   1 need you                                ║
║     ~/db         ████░░░░░░  4 threads    4 agents                                             ║
║     ~/auth       ███░░░░░░░  3 threads    3 agents                                             ║
║     ~/docs       ██░░░░░░░░  2 threads    2 agents                                             ║
║     + 7 more                24 threads    3 agents                                             ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     3 need you                                                                                 ║
║                                                                                                ║
║     Auth Refactor          ~/api       plan         waiting        [approve] [reject]          ║
║     Settings Page          ~/tui       normal       question       [answer]                    ║
║     Payment Flow           ~/api       plan         waiting        [approve] [reject]          ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  click repo to expand · /threads see all · ⇥⇥ recent                                           ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## 4C: Dashboard - Minimal (Maximum Abstraction)

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 78%   4.2g                                                    ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                              47 threads · 32 agents                                            ║
║                                                                                                ║
║                    ████████████████████████████████████████████░░░░░░░░░░                      ║
║                    working                                     idle                            ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     3 need you                                                                                 ║
║                                                                                                ║
║     Auth Refactor                                                                   waiting    ║
║     plan ready · ~/projects/api                                 [approve] [reject] [view]      ║
║                                                                                                ║
║     Settings Page                                                                   question   ║
║     "which auth provider should I use?" · ~/tui                         [answer] [view]        ║
║                                                                                                ║
║     Payment Flow                                                                    waiting    ║
║     plan ready · ~/projects/pay                                 [approve] [reject] [view]      ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  /threads see all · ⇥⇥ recent                                                                  ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## 4D: Dashboard - All Good State (Nothing Needs Attention)

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 45%   2.1g                                                    ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                              47 threads · 32 agents                                            ║
║                                                                                                ║
║                    ████████████████████████████████████████████░░░░░░░░░░                      ║
║                    working                                     idle                            ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                              all clear                                                         ║
║                                                                                                ║
║                         nothing needs your attention                                           ║
║                         32 agents working autonomously                                         ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  /threads see all · ⇥⇥ recent                                                                  ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## 4E: Dashboard - Heavy Load Warning

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ● live   cpu 94%   7.8g   ⚠ high load                                      ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                              127 threads · 89 agents                                           ║
║                                                                                                ║
║                    ████████████████████████████████████████████████████████████████████████    ║
║                    working                                                                     ║
║                                                                                                ║
║                    ⚠ system under heavy load. consider stopping some threads.                  ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     12 need you                                                                                ║
║                                                                                                ║
║     Auth Refactor          ~/api       plan         waiting        [approve] [reject]          ║
║     Settings Page          ~/tui       normal       question       [answer]                    ║
║     Payment Flow           ~/api       plan         waiting        [approve] [reject]          ║
║     User Dashboard         ~/web       plan         waiting        [approve] [reject]          ║
║     + 8 more needing attention                                               [view all]        ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  /threads see all · /stop-all · ⇥⇥ recent                                                      ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

---

# Comparison Summary

| Ideation | Best For | Threads Visible | Complexity |
|----------|----------|-----------------|------------|
| **1: Spacious** | Small workloads, readability | 5-6 | Low |
| **2: Compact** | Medium workloads, density | 13-15 | Medium |
| **3: Hover Actions** | Quick actions without drilling in | 5-15 | Medium |
| **4: Dashboard** | Large workloads (50+), overview | 3-5 detailed + aggregate | High |

---

# Common Elements

## Thread Switcher (⇥⇥) - MRU Based

```
          ╭─────────────────────────────────────────────────────────────────────╮
          │                                                                     │
          │  recent threads                                                     │
          │                                                                     │
          │                                                                     │
          │  ▸ Auth Refactor                        plan         waiting        │
          │    ~/projects/api · 2m ago                                          │
          │                                                                     │
          │    API Endpoints                        exec         ●●●●○○○        │
          │    ~/projects/api · 12m ago                                         │
          │                                                                     │
          │    Settings Page                        normal       question       │
          │    ~/projects/tui · 45m ago                                         │
          │                                                                     │
          │    Test Suite                           exec         ●●○○○○○        │
          │    ~/projects/tui · 1h ago                                          │
          │                                                                     │
          │    DB Migration                         done         verify         │
          │    ~/projects/db · 2h ago                                           │
          │                                                                     │
          │                                                                     │
          │  ───────────────────────────────────────────────────────────────    │
          │  ↑↓ navigate    return switch    esc close                          │
          │                                                                     │
          ╰─────────────────────────────────────────────────────────────────────╯
```

## /threads - Full Search

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ← back · esc                                                               ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     all threads                                                              47 total          ║
║                                                                                                ║
║                                                                                                ║
║     hot                                                                                        ║
║                                                                                                ║
║     Auth Refactor                        plan      waiting       ~/api            2m ago       ║
║     API Endpoints                        exec      ●●●●○○○       ~/api           12m ago       ║
║     Settings Page                        normal    question      ~/tui           45m ago       ║
║     Test Suite                           exec      ●●○○○○○       ~/tui            1h ago       ║
║     DB Migration                         done      verify        ~/db             2h ago       ║
║                                                                                                ║
║                                                                                                ║
║     archived                                                                                   ║
║                                                                                                ║
║     User Auth v2                         done      verified      ~/api        3 days ago       ║
║     Payment Integration                  done      verified      ~/pay        5 days ago       ║
║     Dashboard Redesign                   done      verified      ~/web        1 week ago       ║
║     API Rate Limiting                    done      verified      ~/api        2 weeks ago      ║
║     ...                                                                                        ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │ search threads...                                                                      │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  type to filter · click to resume · ↑↓ navigate                                                ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## /repos - Available Repositories

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ← back · esc                                                               ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     available repositories                                                    12 total         ║
║                                                                                                ║
║                                                                                                ║
║     ~/projects/api                                          8 threads    12 agents             ║
║     ~/projects/tui                                          6 threads     8 agents             ║
║     ~/projects/db                                           4 threads     4 agents             ║
║     ~/projects/auth                                         3 threads     3 agents             ║
║     ~/projects/docs                                         2 threads     2 agents             ║
║     ~/projects/web                                          0 threads                          ║
║     ~/projects/mobile                                       0 threads                          ║
║     ~/projects/infra                                        0 threads                          ║
║     ~/work/client-a                                         0 threads                          ║
║     ~/work/client-b                                         0 threads                          ║
║     ~/personal/side-project                                 0 threads                          ║
║     ~/personal/experiments                                  0 threads                          ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  click to start thread in repo · type to filter                                                ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## Thread Detail - Plan (Waiting for Approval)

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ← back · esc                                                               ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     Auth Refactor                                                                              ║
║     plan                                                                                       ║
║                                                                                                ║
║     ~/projects/api                                                                             ║
║     started 24 minutes ago                                                                     ║
║                                                                                                ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     ready for your review                                                                      ║
║                                                                                                ║
║     nova completed planning. 7 phases identified.                                              ║
║     changes across 12 files.                                                                   ║
║                                                                                                ║
║     last tool: read                                                                            ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  return approve · r reject · s stop                                                            ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## Thread Detail - Executing

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ← back · esc                                                               ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     API Endpoints                                                                              ║
║     executing                                                                                  ║
║                                                                                                ║
║     ~/projects/api                                                                             ║
║     running 12 minutes                                                                         ║
║                                                                                                ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     phase 4 of 7                                                                               ║
║                                                                                                ║
║     ●●●●○○○                                                                                    ║
║                                                                                                ║
║     ✓ research codebase                                                                        ║
║     ✓ create endpoint structure                                                                ║
║     ✓ implement GET handlers                                                                   ║
║     ● implement POST handlers                                                                  ║
║     ○ add validation                                                                           ║
║     ○ write tests                                                                              ║
║     ○ update documentation                                                                     ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  s stop                                                      last: edit src/routes/api.rs     ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## Thread Detail - Ready to Test (Verification)

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ← back · esc                                                               ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     DB Migration                                                                               ║
║     completed                                                                                  ║
║                                                                                                ║
║     ~/projects/db                                                                              ║
║     finished 2 hours ago                                                                       ║
║                                                                                                ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     please verify                                                                              ║
║                                                                                                ║
║     ○  run migrations locally                                                                  ║
║     ○  check user table has new columns                                                        ║
║     ○  test signup flow still works                                                            ║
║     ○  verify existing data preserved                                                          ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  return mark verified · f report issue · a archive                                             ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

## Thread Detail - Has User Question

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║                                                                                ▄▄ ▄▄ ▄▄ ▄▄     ║
║     ← back · esc                                                               ▀  █▀ █▄ █▄▀    ║
║                                                                                                ║
║                                                                                                ║
║     Settings Page                                                                              ║
║     normal                                                                                     ║
║                                                                                                ║
║     ~/projects/tui                                                                             ║
║     active 45 minutes ago                                                                      ║
║                                                                                                ║
║                                                                                                ║
║     ───────────────────────────────────────────────────────────────────────────────────────    ║
║                                                                                                ║
║     waiting for your answer                                                                    ║
║                                                                                                ║
║     "which auth provider should I use for the settings page?                                   ║
║      I found both OAuth and JWT implementations in your codebase."                             ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ──────────────────────────────────────────────────────────────────────────────────────────    ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮    ║
║  │                                                                                        │    ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯    ║
║  return open thread to answer · s stop                                                         ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

---

## Navigation Reference

| Action | How |
|--------|-----|
| open thread | click |
| go back | `esc` or click `← back` |
| recent threads | `⇥⇥` (double-tab) |
| search all | `/threads` |
| see repositories | `/repos` |
| scroll | mouse scroll |
| approve | `[approve]` button or `return` in detail |
| reject | `[reject]` button or `r` in detail |
| stop | `[stop]` button or `s` in detail |
| verify | `[verify]` button or `return` in detail |
| report issue | `[issue]` button or `f` in detail |
| archive | `[archive]` button or `a` in detail |
