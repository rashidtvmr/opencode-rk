# Shared ChatGPT Conversation Transcript

**Source URL:** https://chatgpt.com/share/e/6ae1f178-6dd0-83ee-807a-591a9be0f06a
**Conversation title:** Parallel Agents Failure Analysis
**Retrieved:** 2026-10-28 via cua_repl computer use (Google Chrome AX tree)
**Investigator:** @xkiro-mimo-v2-6-flash (session ses_e7d5fe66bffeKqPiz4k1qdHadM)
**Method:** Accessibility tree extraction; no webfetch fallback used

---

## Turn 1 — User ("You said:") — Monday 11:15 PM

So If you see our opencode-rk using [Chat On Steroids Core](https://chatgpt.com/plugins/plugin_asdk_app_6aad45f46a6c8191a85d68d7df819a55) you can see the intial foundation of that project was built by you only

So we thoguht just writing task and looping subagents to work on it will lead to faster result but it didnt went like that, instead we got stuck at 600+ worktree and no way to merge those branches taking months to do so

So based on peoples recommendation around the internet and real experiences (now we also know this wont work like that) even though we dont habe resources or token limitation we werent able to complete it as intended

So please look how openai (for match problems) or anthropic (for mograting bun to zig or creating cpp compiler and all) and other people using it successfully

Can you reseaech all and compare ours and exactly pin point what went sideways so our project failed and what we should have done otherwise and in future projects to get it done right and complete it successfully while making use multiple parallel autonomous subagent which not only work on creating blind mew feature worktree but also review, merge and test as each features are complete so project is buolt successfully

*("Worked for 13m 39s")*

---

## Turn 2 — ChatGPT ("ChatGPT said:")

Yes—the approach we took needed a much stronger mechanism for turning parallel work into a finished product. Your criticism is fair.

The assistant-led workflow should have owned implementation, review, integration, and acceptance from the beginning. Leaving you with hundreds of branches to reconcile was a failure of that workflow.

After inspecting your repository through Chat On Steroids and researching the first-party accounts, my main finding is:

The project generated work faster than it could establish that the combined application worked. Then the attempts to recover introduced additional coordination and bookkeeping work.

There is more to this than "too many agents" or "we forgot to add a reviewer." Your repository already contains several generations of integration rules, independent review requirements, and automated controls. Some are sensible. The problem is how the whole process evolved—and whether its completion signals corresponded to the product you wanted.

I'll separate what I verified from my interpretation, then give you a concrete operating model for future projects and the recovery approach I would use for opencode-rk.

### 1. What I actually found in your project

#### The current repository is more complicated than its folder names suggest

These are observations from this inspection, not estimates:

| Item | What I observed | What it means |
|------|----------------|---------------|
| Registered Git worktrees | 328 | This is the current registry count, not a count of active features. |
| Local branch references | 538 | Branch references, worktrees, and distinct unfinished changes are different quantities. |
| Actual `main-v2` branch | `6f6872bf87e02f05afe9c1a5f63e55fe4c040499`, dated October 26 | This was the current branch tip during inspection. |
| Where that branch is checked out | `/Users/mymac/Projects/opencode-rk-recovery-20261005` | This is the canonical worktree identified by the recovery configuration. |
| Folder named `opencode-rk-main-v2` | Detached at `7256f93d…`, dated October 5 | Its name does not identify the latest `main-v2` code. |
| Recovery acceptance registry | All seven Phase 1 gates recorded as UNPROVEN; registry metadata last updated October 21 | That registry does not certify the newer October 26 build. It does **not** prove those features fail today. |

Your historical "600+" figure may reflect an earlier snapshot or a different inventory. I cannot verify that peak from the current registry, and I am not dismissing it. The integration burden is clearly substantial either way.

The canonical worktree also contains ongoing, uncommitted work. I preserved it.

This was a retrospective investigation, not a fresh release certification:
I read historical test reports but did not rerun every branch or the complete release suite.

#### The most revealing evidence is historical

Three records explain much more than branch counts:

- **September 16 audit:** 22 task cards referenced crates that did not exist, while implementations often existed under different module names. It also recorded stale progress files and uncommitted module wiring.
- **September 16 acceptance proposal:** reported 1,494 crate tests passing, while some proposed acceptances still carried caveats such as unwired functionality or browser behavior not executed.
- **October 1 merge report:** classified 221 frozen eligible tips into 14 integrated candidates and 207 quarantined tips, while explicitly leaving runtime acceptance and the broader release open.

Those are recorded findings in:
- `/Users/mymac/Projects/opencode-rk-main-v2/worklog/AUDIT-NONACCEPTED.md`
- `/Users/mymac/Projects/opencode-rk-main-v2/worklog/ACCEPTANCE-FLIP-PROPOSAL.md`
- `/Users/mymac/Projects/opencode-rk-main-v2/worklog/MERGE-STRATEGY-20261001.md`

The counts need careful interpretation: 207 quarantined tips do not necessarily mean 207 useful features awaiting merge; 1,494 passing tests do not necessarily mean the application's full user journey worked.

The central problem was that these different kinds of progress had become difficult to reconcile.

### 2. Where the approach went sideways

#### A. We decomposed work at a level that made parallel coding easier than product assembly

An older instruction in:
`/Users/mymac/Projects/opencode-rk-convergence/AGENTS.md`
tells delegated lanes to own exactly one file.

The later instructions explicitly say to avoid splitting a behavior into one task per file. That is a meaningful change in strategy.

File ownership can prevent editing collisions, but a feature in your application often crosses several boundaries:

- Provider response parsing.
- Session state.
- Event delivery.
- Tool execution.
- Permission handling.
- Persistence.
- TUI or web rendering.

For example, "cancel a running tool and successfully submit the next prompt" cannot be established by completing a cancellation module alone. The cancellation must reach the process, release session ownership, update persisted state correctly, and leave the client usable.

The September audit's uncommitted lib.rs wiring is consistent with this problem: pieces existed, while assembly remained separate work.

**My diagnosis:** early decomposition encouraged the production of locally testable pieces faster than the completion of user-visible behavior.

This does not mean every task must be a large feature. Independent libraries and small fixes are perfectly valid tasks. The requirement is that each task has an explicit route into an integrated behavior.

**What we should have done:** build a small working application path first, then delegate increments against that running baseline. Where file-level tasks were necessary, their parent integration task should have remained actively owned and unfinished until the complete behavior passed.

#### B. The plan and the implementation stopped describing the same system

This is one of the clearest, directly documented defects.

The September audit says task cards referenced nonexistent crates such as:

- `crates/ext`
- `crates/share`
- `crates/delegation`
- `crates/autonomy`
- `crates/integration`

Implementations frequently existed elsewhere, as modules inside other crates.

Imagine a task whose acceptance command is effectively:

```bash
cargo test -p opencode-rk-ext
```

If that crate does not exist, repeatedly improving an implementation inside another crate will never make that command meaningful.

That is a coordination defect with a concrete consequence: the worker, the implementation, and the verifier disagree about what is being built.

The same audit recorded FEATURES.md and ralph.json disagreeing, with acceptance blocked by validation failures.

**What we should have done:** reconcile the architecture and task mapping as soon as this divergence appeared. Preserve the behavioral requirements, but allow an independently reviewed correction to stale paths, package names, and test commands.

Freezing semantic requirements is useful. Freezing an incorrect description of the repository makes the process impossible to satisfy legitimately.

#### C. "Tests passed" existed at several different levels, without a dependable bridge between them

The acceptance proposal reported 1,494 passing crate tests. It also proposed acceptance for some entries with caveats involving unwired paths or unexecuted browser behavior.

The document explicitly says PROPOSAL ONLY, so I am not claiming those entries were actually accepted. The revealing point is that its evidence and its proposed completion status were operating at different levels.

There are several distinct claims:

| Claim | What it establishes |
|-------|-------------------|
| The implementation file exists | Someone produced code. |
| The crate compiles | Its types and dependencies satisfy that compilation. |
| Focused tests pass | The tested behavior works under those conditions. |
| The branch passes its checks | That candidate works against that particular base. |
| The combined candidate passes | The proposed integration works with the current target. |
| The installed application passes the user journey | The delivery meets that acceptance scenario. |

Moving between these levels requires additional evidence.

**What we should have done:** make the final level explicit for each feature, and have the same task remain open through the necessary integration checks. A worker's report should attach evidence; it should not be able to redefine completion.

#### D. Integration became a large downstream project

The October merge report is evidence that integration had become an undertaking in its own right: hundreds of tips required classification, preservation, preparation, quarantine, or selective integration.

At that point, merging is no longer primarily a Git operation. Someone has to determine:

- Whether two branches implement the same requirement.
- Whether one supersedes another.
- Whether a change assumes an obsolete interface.
- Whether an earlier fix has already been copied or cherry-picked.
- Whether passing tests still represent the current intended behavior.
- Whether combining two individually sound changes breaks a shared invariant.

That work is much harder after the original implementation context has aged.

Unlimited tokens do not remove this constraint. Consider an illustrative example, not a measurement of your project:

| Daily activity | Changes |
|---------------|---------|
| Workers produce candidates | 30 |
| Review, integration, and acceptance complete | 8 |
| Unfinished inventory grows | 22 |

After 30 days, that is 660 additional unfinished candidates.

Adding more implementation workers accelerates the accumulation until the downstream bottleneck improves.

**What we should have done:** control the amount of unfinished work admitted into the system and direct spare agents toward the bottleneck. Integration capacity includes review, builds, interaction testing, and repair—not merely permission to run `git merge`.

#### E. Recovery accumulated its own process overhead

The Git history shows several rounds of correction:

- September 18: task claims and handoff protocol.
- September 19: explicit convergence and completion rules.
- September 30: Convergence V2.
- October 5: recovery activation and controller hardening.
- October 21: an explicit instruction to prioritize Phase 1 over orchestration bookkeeping.

The current recovery instructions preserve your October 21 direction that product implementation and installed verification must not be blocked on guard scripts, queue transitions, controller migration, or receipt ceremonies.

That is documented in:
`/Users/mymac/Projects/opencode-rk-recovery-20261005/AGENTS.md`

This does not establish that every guard was wrong. It does show that recovery machinery had become obstructive enough to require an explicit change in priority.

**My diagnosis:** the response to coordination failures partly became more coordination infrastructure. That can be necessary, but it becomes counterproductive when it consumes effort without reducing failures in the application.

**What we should have done:** distinguish product correctness checks from administrative consistency checks, assign owners to both, and prevent stale administrative representations from indefinitely blocking demonstrably correct product work.

#### F. There are several competing representations of "current"

The folder/branch mismatch is a concrete example. Another is the gap between the recovery registry and newer work.

Your current controls are not merely aspirational. I inspected:
`/Users/mymac/Projects/opencode-rk-recovery-20261005/config/recovery-mode.json`
and the corresponding controller code. They include fixed lanes, a four-candidate threshold, independent review, and acceptance tied to the integrated commit.

So the diagnosis is not "you never implemented controls."

It is more precise:
Controls arrived after substantial integration debt, and the repository now contains several layers of historical and current state. Their existence alone does not establish that every active workflow passes through one reliable completion mechanism.

The October 21 override also means stale formal bookkeeping cannot automatically be treated as a violation or a current product failure.

**What we should have done:** maintain one authoritative identity for each task and candidate, with other summaries derived from it. Every report must say which commit—and, where relevant, which uncommitted working tree or installed binary—it actually examined.

### 3. What the successful examples actually did

I prioritized accounts written by the engineers involved and their public repositories. These are valuable operational reports, but most are first-party reports rather than controlled, independently reproduced productivity studies.

Two corrections first: Bun's migration was **Zig to Rust**. Anthropic's compiler experiment produced a **C compiler written in Rust**, rather than a general C++ compiler.

| Example | What made the work converge | Qualification that matters |
|---------|---------------------------|--------------------------|
| **Bun migration, July 2026 account** | At peak, 64 Claude sessions operated across four worktrees. Porting rules, independent adversarial reviews, and the existing TypeScript test suite supplied shared constraints. The author repeatedly adjusted the workflows. | This was a staged migration, with translation, compilation, execution, and parity phases. It was not a policy of keeping every intermediate edit runnable. [Bun Blog](https://bun.com/blog/bun-in-rust?utm_source=chatgpt.com) |
| **Anthropic C compiler, February 2026** | Sixteen agents used task locks and pulled, merged, and pushed changes as part of completing work. The researcher invested heavily in tests and redesigned verification when agents collided on the same Linux failure. | It was a capability experiment with acknowledged limitations. The repository's human-written notice warns that correctness was not validated sufficiently for reliance. [anthropic.com](https://www.anthropic.com/research/building-c-compiler?utm_source=chatgpt.com) |
| **OpenAI's agent-built internal product, February 2026** | OpenAI reported roughly 1,500 opened-and-merged PRs over five months. Agents could run the app, inspect UI and logs, review each other, repair problems, and complete PRs. | Engineers still shaped the environment, priorities, and acceptance criteria. The result depended on repository-specific tooling. [OpenAI](https://openai.com/index/harness-engineering/?utm_source=chatgpt.com) |
| **OpenAI Symphony, April 2026** | Durable issues, explicit dependencies, bounded execution, and reconciliation separated ongoing work from individual model sessions. Agents could inspect CI and review feedback. | The public reference workflow includes a human-review handoff before landing. Fully autonomous approval would be a deliberate policy adaptation. [OpenAI](https://openai.com/index/open-source-codex-orchestration-symphony/?utm_source=chatgpt.com) |
| **Cursor's browser research, February 2026** | Cursor iterated through failed coordination designs, then used scoped ownership and structured handoffs. It removed a central integrator that had become a bottleneck. | The browser was a research project, explicitly not intended for external use. The experiment allowed bounded transient breakage to increase throughput. [Cursor](https://cursor.com/blog/self-driving-codebases?utm_source=chatgpt.com) |
| **Stripe Minions, February 2026** | A deterministic workflow controlled operations such as linting and pushing, while agents implemented and repaired. Stripe reported over 1,300 merged agent-produced PRs per week. | Those PRs were human-reviewed. CI repair attempts were bounded rather than allowed to loop indefinitely. [Stripe Dot Dev Blog](https://stripe.dev/blog/minions-stripes-one-shot-end-to-end-coding-agents-part-2?utm_source=chatgpt.com) |
| **Dropbox Nova, May 2026** | Sessions began from explicit commits, received validation commands, and were restricted to one branch. Publication happened outside the agent. | Dropbox deliberately constrained branch management so testing, rebasing, and ownership stayed predictable. [Dropbox](https://dropbox.tech/machine-learning/introducing-nova-our-internal-platform-for-coding-agents?utm_source=chatgpt.com) |
| **StrongDM's software factory, February 2026** | Its reported autonomous workflow emphasized end-to-end acceptance scenarios, including scenarios outside the implementation repository. | Its claim of operating without human code review is its own reported model. The useful mechanism is independently controlled acceptance evidence. [factory.strongdm.ai](https://factory.strongdm.ai/?utm_source=chatgpt.com) |

The compiler experiment did not need a central orchestration agent. Cursor removed a central integration bottleneck. Bun used staged migration phases. Stripe retained human review.

#### Why the OpenAI mathematics comparison needs care

OpenAI's reported large mathematics experiment used groups exploring different approaches, human resource reallocation after promising intermediate results, and consolidation of useful findings across groups. It was substantially more directed than continually spawning identical independent attempts.
[OpenAI: On the Navier–Stokes Millennium Prize Problem](https://openai.com/index/navier-stokes-solution/?utm_source=chatgpt.com)

But mathematical search and software construction compose differently.

In a search, many attempts can be discarded while one successful argument is retained. In an application, the session engine, storage, tools, permissions, and UI must agree with one another. You cannot simply choose the best subsystem independently if it assumes a different contract.

Verification itself also needs scrutiny. An October research preprint disputes whether the Lean formalization in one prominent OpenAI mathematics claim faithfully represents its natural-language argument. I am not adjudicating that dispute here; it illustrates why "the checker passed" must remain connected to "the intended problem was checked."
[arxiv.org: Navier-Stokes lost in translation](https://arxiv.org/abs/2610.08144?utm_source=chatgpt.com)

For your project, the equivalent danger is a test that passes while exercising a substitute implementation, bypassing the actual caller, or checking a weaker requirement.

### 4. The operating model I would use for future projects

Keep multiple autonomous agents. Make their responsibility extend through delivery.

The most important change is:
**A task remains owned until its required behavior is accepted in the integrated application, or it is explicitly rejected or superseded.**

An agent session can end. Ownership of the work must survive it.

#### 4.1 Establish a small, executable product contract before broad expansion

- Start the application.
- Create or resume a session.
- Send a prompt and stream a response.
- Receive a tool request.
- Approve or deny it through the actual permission mechanism.
- Persist the result and continue.
- Cancel correctly.
- Restart and recover the session.

That path establishes the interfaces through which later work must connect.

Your existing Phase 1 contract already describes a substantially similar local-native journey:
`/Users/mymac/Projects/opencode-rk-recovery-20261005/docs/PHASE1_ACCEPTANCE.md`

For a port, also pin the reference implementation and classify each requirement as:
- Required upstream behavior.
- An explicitly approved difference.
- An additional feature.
- Deferred release scope.

Anthropic's migration guidance is particularly relevant here: establish a judge that evaluates source and target implementations on equal terms, and verify that it rejects deliberately broken implementations.
[Claude by Anthropic: How Anthropic runs large-scale code migrations with Claude Code](https://claude.com/resources/articles/ai-code-migration?utm_source=chatgpt.com)

#### 4.2 Separate model judgment from mechanical workflow enforcement

Use agents for reasoning:
- Understanding requirements.
- Designing implementations.
- Reviewing changes.
- Diagnosing failures.
- Resolving semantic conflicts.

Use ordinary software for facts and transitions:
- Which task is owned.
- Which dependencies are satisfied.
- Which commit is the base.
- Which candidate was reviewed.
- Whether required commands ran successfully.
- Whether the canonical branch moved.
- Whether a task may advance.

A model should not need to remember to enforce a concurrency limit. The scheduler should refuse further admission when that limit is reached.

OpenAI's Symphony specification is useful because it treats claims, concurrency, reconciliation, and retries as explicit orchestration responsibilities. It does not require an LLM to keep the entire operational state in its conversational memory.
[GitHub: symphony/SPEC.md at main · openai/symphony](https://github.com/openai/symphony/blob/main/SPEC.md?utm_source=chatgpt.com)

#### 4.3 Keep one compact task record

The record should contain only information that changes execution:

| Field | Purpose |
|-------|---------|
| Stable task ID and observable outcome | Defines the work independently of a chat session. |
| Dependencies and contract/reference version | Prevents work starting against unresolved assumptions. |
| Owner and allowed changes | Establishes responsibility and avoids competing writes. |
| Base commit and candidate commit | Makes review and test evidence reproducible. |
| Acceptance scenarios | Defines what completion means. |
| Current state, blocker, and evidence | Supports continuation after interruption. |

Status reports, dashboards, and feature summaries should derive from this record.

Do not manually maintain five different representations of whether a feature is complete.

#### 4.4 Make review and integration part of the normal lifecycle

The critical details are:
- Review applies to an identified candidate.
- Material changes after review require corresponding review again.
- Integration failures return to an accountable owner.
- Dependencies unlock after the relevant acceptance condition.
- A completion message does not itself advance the task to accepted.

This is what prevents "the coder finished" from becoming "someone now needs to rescue its branch."

#### 4.5 Use a controlled merge queue without making one LLM review everything

The queue should:
- Take a reviewed candidate.
- Combine it with the current target and applicable preceding changes.
- Run required checks on that combined state.
- Publish only the state those checks covered.
- Rebuild or revalidate when the relevant base changes.
- Return failures to the candidate's owner with concrete evidence.

Reviews and validation preparation can proceed in parallel.

GitHub's merge queue documents this distinction: it tests changes against the latest target plus preceding queued changes. If using its Actions integration, the required checks must handle `merge_group`. Equivalent semantics can also be implemented with existing local or CI tooling.
[GitHub Docs: Managing a merge queue](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue?utm_source=chatgpt.com)

For your project, I would retain the single publication authority while reducing the amount of reasoning that must happen serially inside it.

#### 4.6 Bound unfinished implementation, while keeping many agents useful

Your existing four-lane/four-waiting-candidate policy is a reasonable starting point. The important measurement is unfinished work across the pipeline, including candidates waiting for review or validation—not merely those already labelled integration-ready.

You can still use around 20 worker assignments effectively:

| Assignment type | Illustrative slots |
|----------------|-------------------|
| Implementers on independent behaviors | 4 |
| Independent reviewers | 4 |
| Acceptance-scenario and test preparation | 4 |
| Upstream contract investigators | 3 |
| Integration preparation and conflict diagnosis | 2 |
| Adversarial failure investigation | 2 |
| Coordinated build/validation execution | 1 |

The lead coordinator is additional to that illustration. These are starting allocations, not research-established optimal numbers.

Your current configuration serializes heavy validation. Four test-preparation agents therefore should not automatically launch four heavyweight Cargo or PTY runs.

When the candidate backlog fills, implementation capacity should move toward draining it. When fewer independent tasks exist, do not manufacture additional tasks to keep a numerical target occupied.

Many agents can contribute to four active changes without creating twenty permanent feature branches.

#### 4.7 Give the verifier independent authority over behavior

An independent review should examine the actual change, relevant contracts, and execution evidence. Giving another agent only the author's confident summary provides weak independence.

Key verification scenarios include:
- Denying permission causes no tool side effect.
- Cancellation reaches the running operation and leaves the session usable.
- Attaching a second client does not duplicate execution.
- Restart preserves typed tool history without replaying uncertain side effects.
- Provider fixtures exercise the real application wiring.
- Terminal state is restored after failure, interruption, and exit.

Use deterministic local provider fixtures for repeatable scenarios, with separate real-provider checks where needed.

Also distinguish three outcomes:
- **PASS:** the requirement was exercised and satisfied.
- **FAIL:** it was exercised and violated.
- **BLOCKED/UNPROVEN:** it could not be evaluated.

Missing credentials, an unavailable platform, or a broken test harness should not become fabricated success or be counted automatically as an application defect.

Anthropic's long-running-agent research similarly found that agents could declare completion prematurely and miss failures that only appeared through actual end-to-end interaction.
[anthropic.com: Effective harnesses for long-running agents](https://www.anthropic.com/research/effective-harnesses-for-long-running-agents?utm_source=chatgpt.com)

#### 4.8 Make retries improve the approach

A repeated attempt should bring new information:
- A smaller reproduction.
- A corrected contract interpretation.
- A different implementation strategy.
- Better diagnostics.
- An independently identified cause.

After several failed candidate cycles, pause that approach for diagnosis. Your existing configuration already specifies a strategy review after three failed cycles.

The next action should not automatically be another descendant branch with another "fixed-final" name.

Stripe's bounded CI loop is one concrete example of refusing to equate unlimited retries with useful autonomy. Its exact retry count need not become your policy, but the principle transfers.
[Stripe Dot Dev Blog: Minions Part 2](https://stripe.dev/blog/minions-stripes-one-shot-end-to-end-coding-agents-part-2?utm_source=chatgpt.com)

#### 4.9 Measure delivery and quality

The dashboard should emphasize:
- Accepted user behaviors on the integrated build.
- Time from task start to accepted integration.
- Age of the oldest unfinished candidate.
- Time waiting for review, builds, or integration.
- Regressions and reverts.
- Reopened tasks.
- Release-critical scenarios still unproven.

Agents spawned, tokens consumed, files generated, and branches created are operational inputs. They do not establish delivery.

Increase writer concurrency only while it improves accepted output without causing candidate age and regression rates to deteriorate.

### 5. What I would do with opencode-rk now

I would use the Phase 1 recovery objective you already approved. I would not make "merge every historical branch" the definition of success.

#### Step 1: Establish one explicit current release candidate

Use the actual canonical worktree and identify the exact source state being evaluated.

Preserve ongoing uncommitted work and distinguish it from the committed candidate. Ensure any installed binary being tested is traceable to that candidate.

The misleading folder name should cease to be an authority; the branch, commit, and build identity should be explicit in every assignment.

#### Step 2: Re-establish the current Phase 1 result

Run the existing local-native journey on that candidate and produce a fresh result for each requirement.

Record the observed failures. Those become the active recovery queue.

#### Step 3: Assign complete repairs to those failures

For each observed failure:
- Give one owner the behavior to repair.
- Supply the relevant upstream contract and current base.
- Allow the files needed for that behavior.
- Obtain independent review.
- Validate the prepared integration.
- Land it and prove the integrated behavior.

Keep the task owned throughout.

#### Step 4: Use historical branches as a source of reusable work

For each current requirement, determine whether an old branch contains useful implementation, tests, or diagnostic evidence.

Useful dispositions include:
- Already present.
- Useful implementation salvaged.
- Useful tests salvaged.
- Duplicate.
- Superseded.
- Conflicting alternative preserved.
- Process or historical evidence only.

Git ancestry alone is insufficient: cherry-picking can preserve content without original ancestry, while ancestry-preserving merges can retain history without activating its content.

Sometimes applying a small known fix to the current architecture will be more reliable than reconciling an entire old branch. Preserve the original work either way.

#### Step 5: Finish the agreed release before reopening the whole roadmap

Your Phase 1 contract deliberately targets the local native macOS application. It explicitly defers full browser qualification, Ubuntu release qualification, remote control, mobile clients, and broader parity.

That is a useful release boundary. Those goals remain on the roadmap, but they should not silently expand the completion condition of the current release.

#### Step 6: Expand only after the delivery loop demonstrates that it closes

Once the local-native release is demonstrated, use the same mechanism for web, Linux, and subsequent features.

The evidence that the process works is a repeatable sequence:
A requirement enters, an integrated behavior is accepted, its temporary work is retired or reused, and the application remains usable.

### 6. What I should have insisted on from the beginning

The assistant-led approach should have established four things before scaling implementation:

1. **A runnable application path**, so new work had somewhere concrete to connect.
2. **An authoritative behavioral judge**, so agents could distinguish progress from plausible output.
3. **A bounded integration pipeline**, so unfinished work could not grow indefinitely.
4. *(content continues — see COVERAGE.md for truncation note)*

---

*End of transcript as captured from AX tree. The final numbered item in section 6 may continue beyond the captured viewport.*
