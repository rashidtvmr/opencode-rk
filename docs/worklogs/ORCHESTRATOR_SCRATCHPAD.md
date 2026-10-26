# Orchestrator scratchpad

Updated: 2026-10-26. Session: `ses_ef3b17240ffeYKnIlsBn552l4m`.

This is a durable reasoning/handoff note, not an acceptance ledger. Revalidate
source hashes, active runtime state and raw logs on resume. Keep this file current
at meaningful transitions and before compaction. Apply the exclusive allowlist
and operating rules at the top of `AGENTS.md` before dispatching more work.

## Product objective and authority

- Complete the working native Rust OpenCode application, faithful to OpenCode V2
  plus approved extensions; Phase 1 is the immediate release priority.
- Canonical checkout: `/Users/mymac/Projects/opencode-rk-recovery-20261005`.
- Latest product HEAD: `b797acda85109619b44d142f6a9aa2136fa8f1ca` (`main-v2`).
  Includes PID reuse c821e522, mouse-selection5cdbb989, model subsequencebace0c71,
  and session wrap/refreshb797acda. Exact picker filters PASS on this SHA;
  installed binary remains older/unupdated. Documentation-only commits may follow.
- User-selected upstream: `/Users/mymac/Projects/tmp/opencode`, verified HEAD
  `450e7b92412c780a7b5a75226bb8d49aaca24e22`. The older lock pin `95daf906...`
  is not the selected reference for current parity work.
- Sole integration writer: main orchestrator. Preserve all legacy refs, user work,
  databases and preferences. Preserve user daemon PID 73389 on port 4096.

## Immediate continuation — user-selected context UI

### Continuation after checkpoint: actual validation and integration

Post-checkpoint 2026-10-26: actual HEAD eba78312 (measured usage footer click
integration); exact composed candidate native CLI **858 PASS** prior integration,
source hashes checked on all three canonical files before scoped commit. Broader
suite lacked target regressions: private06-pointer test overlay ran **1 PASS,
1 FAIL** (stale target remained active after open_page). Main working native_view
adds Page::Chat eligibility gate; same frozen tests rerun **2 PASS** in
ready-cli-validation-4n3v0ij9/context-pointer-guard.*. Original reviewer12 renewed
review session ses_e882a160fffe9IHVmD1urKIdL8, guard UNCOMMITTED until exact approval.
This finding remains distinct from real installed mouse journey, still unexecuted.

Markdown candidateebb7a4e... now original reviewer28 **APPROVE** a7f614fb... and
main exact37testsPASS; no Markdown product integration yet. Grid v4ec1c403e...
originalnegative reviewer13 **APPROVE** fc992eea... and main11testsPASS; v3 rejection
retained. Renderer live integration remains pending01 author's final artifact.

Recent replenishment (all privatewrites, noheavygrants, nofallback, usageUNKNOWN):
- ses_e881cc726ffe8M5wbG95rNoll7 xkiro-gpt-6-luna117-keybindruntime replaces
  repeated86no-codehandoffs; realmodule/wiring/tests required.
- ses_e881c36cbffe1UkvtUVPVczwBd xkiro118-homecurrent replaces113corruptpatch;
  approvedHomecomposition must preserve currenteba783pointer/leader, newreview.
- ses_e881b71f1ffeHQc8PXf6JY8Rmw xkiro119-accountingmerge prepares scopedindex
  patch and separateprivatecomposition preservingunknownuserdirtylib.rs work.
  Exactbackendv3 approveda60b730e... +294serverPASS, NOTintegratedyet.
- ses_e881b2eeaffeq4WBfHWdMN3s3E luna120-releaseprep exactpackaging/provenance/
  disposableinstalledcommands read-only; no user install or daemon mutations.
- ses_e881ec694fferRLzonJpRsgxHU luna116-tabhoverreview evaluatesactual108worker
  path /T/opencode/108-tabhoverresize (NOreplenish20prefix). Patch88e35f54...
  sourcee58f761f... base1c639; currentcontextpointercompositionstillneeded.
Mainheavy slot FREE after two targetedpointertestsPASS. Preserveunknownweb/server
edits anddaemon73389. Currentactivecountnotfullyaudited; do not claim20 from
launchhistory, replenishindependentusefulassignments withoutduplicatewriters.

Route availability change: 9router-luna-free now reports `[codex/gpt-6-luna]
rule luna-free left no eligible account` on consumer08, pointer12 re-review,
decoder77 composition, release120, and hover116. STOP dispatching this route;
unfinished work redirected explicitly to authorized xkiro-gpt-6-luna or
vyce-deepseek-v41 (never automatic fallback). All cumulativeusage UNKNOWN.
GLM/combined/TokenHarbor earlier pauses remain in force. No budget reset inferred.

Actual fresh validations:
- Chat114 patchc101ab5... applies ontoeba783 but targeted tests **1 PASS 2 FAIL**,
  duplicated #[test] registration of Unicodecase; validexpectedSome4,28 gotNone.
  Author ses_e88238007ffel7KrMpn0CE2H2v resumedv2, oldcandidate NOTready.
- Keybind117 realmodule73681906... privatecompile **3 PASS**, runtimewiring is
  STILLPROSEONLY. Independentreview125active; do notcallmodulecompletefeature.
  CorrectCLIcommand always --bin oc2 --features native (no --lib target).
- Main SERIALIZED heavy slot NOW releasebuildshell sh_177e8c29b001ezlc1745UN1Ma6:
  disposable archiveexacteba783, cargo+1.98.1 build --release -popencode-rk-cli
  --featuresnative --locked --offline --binoc2, jobs1. Testoverlaysremovedfrom
  main/view/entry. Awaitnotification; no otherCargo/PTYgrant. Userbinnotinstalled.

Replacement/useful assignment roster added (all private artifacts/noheavy):
- xkiro121pointerguardreview ses_e881a52faffeGWMOtHe4OPg7hD (workingguardexacthash)
- xkiro15renderfinish ses_e881a0128ffeOsgD3p4KE1bRXf (author01completedstalehashes;
  exactbackendv3+approvedgridv4+actualpainter composition required)
- xkiro16consumerfinish ses_e8819c868ffevKo7wZPdNHwihG (92predicate actualconsumer)
- xkiro122hoverreview ses_e88199881ffeYXC3ZAFQpj7TCA (replacefailed116)
- xkiro124decodercompose ses_e88190d52ffepH4INVkeTW1rsA (replacefailed77)
- xkiro14contractaudit ses_e881a871bffeAx1Q3NGF5RWD8X (snapshotproducer/parser)
- vyce123releaseprep ses_e88193f71ffemwvved0jzPYf5P (replacefailed120)
- vyce17endpointtests ses_e8818c3acffes2dx2ndmeVMl9n (actualHTTPcoverage)
- vyce125keybindreview ses_e8818427cffex9w1UZloCoyekU (moduleupstreamsemantics)
- vyce126toolcurrent ses_e88178e30ffeGmFE4m3sCeHENY (26PASSapprovedintegrationpatch)
- vyce127modelsdevaudit ses_e8816fce5ffep0siTAqHefQuNw (runtimecatalogexactflow)
- vyce128authedgeready ses_e8816c9ebffeesK73UBx7JjBtP (approved12testsnewfileprep)
- xkiro129permissionsave ses_e88169771ffecjl1GyABDzsjSE (trustedrequestsaveparity)
- xkiro130shellverifiedwiring ses_e88165807ffeg78c6cCGW7N4GE (typedcallbackatverified
  brokerpoint; explicitnewtools/serverprivategrant, no proofpresenceapproval)
Previousactive115arrowfix/118homecurrent/119accountingmerge remain independent.
Vyce route six listed unfinished assignments, below eight concurrentlimit;
exclude completed99diagnosis from activecount. Track completions promptly.

Latest actual release build eba78312 SUCCEEDED. Binary (not installed):
`context-exact-integrated-jjhruynv/target/release/oc2`, SHA256
6790ff2352f99c96b9c952e86e4be5d4f297e7253de7b5913b67a533644eb779.
Receipt ready-cli-validation-4n3v0ij9/release-build-receipt.json. This binary lacks
pending completehitguard/dashboard/Home/Kitty repairs; not finalreleaseacceptance.

Pointer121 originalnewreview NEEDS_FIX working622219...: PageChat doublesasHome.
Mainnowadds session_route eligibility ANDclearscontext_usage_hit onroutechange,
withtwocanonicalregressions (geometry/page andunknownzero/Homebeforepaint/resize).
Reviewer121 resumedv2 exactnewhash. FullCLIvalidationnow shell
sh_177ebf77d001TprV8Mq6s2K4Qu oneba783archive+workingnative_view ONLY. Other
privateoverlaysremoved; decoderbytecomparedtocleanbase. Resultpending, noheavy
otherworkergrant. Do not use earlier2PASSascandidate resultfornewtests.

Unexpected canonical native_input_decoder.rs dirtydiff now observed (38lines:
TabNext/Previous/Slot declarations/Kitty mappings) after earliercleanstatus.
Provenance UNCONFIRMED; treataspotentialuserwork, preserve/DO NOTstage/reset.
CurrentworkingTUIlacksnewvariants so canonicaldirtytree notattestedbuildsource.
All mainvalidationisolatedgitarchives exclude thisdiff. Exact115authorhandoff
hasincorrectbaseSHA/truncatedhashclaims; reviewcurrentbasecompositionbeforeuse.

119accountingmerge privatecomposition successfulpreservinguseragentstreamroute;
privatecomposedlib7ed71577... (differentfromreviewedcleanlibf7ac...). User
daemon_authuntouchedfb7f8ac6...; newindependentreview andcompositiontestsneeded.
118Homecurrent validpatch4c9beafc... source9106ef3b... preservesleader/context;
review131active ses_e8815a07cffeqM6RLRI2QQr7az, oldapprovalnottransferred.

122hoverreview NEEDS_FIX: painterdrawsALLstructuralclose_rects sohoverrowtextfix
aloneinsufficient. Author108 resumedv2expandednative_tabs+entryprivategrant,
actualpaintvisibilityneededwithoutbreakingstableSessionRefclosehitgeometry.

15dashboardv1candidate6ada... actualbackendpathclaimunavailablewasfalse (main
compiled294testsfromthosepaths); authorresumedv2exactproducerpaths provided,
musthandlecategoriesarray/coverageobject/model_id/reported_used_tokens/etc.
14audit usedOLDrejected9c0snapshotdespiteaskedlatest; fieldmatrix83a... useful
backendshapeonly, notverdictagainstnewb91... parser. Do not repeatold-sourceclaims.
16goldenconsumernewcf3ffce3... declares92predicatecoverage, UNEXECUTED/APIgap;
needsactualrenderercompositioncompileandindependentreviewnotclaimedPASS.

Keybind117module736819... main3PASS;125reviewmoduleAPPROVEbutitsdurationoverflow
hardeningnote is hypothetical (MAX_SAFE9e15 < i64max9e18; as_millis isu128).
No newtimeoutcapauthorized; exactelapsed-safeMAXbehavior remainsrequired.
RuntimewiringstillMDproposal, nofeatureacceptance. Priornegativeprose-onlykept.

Main completepointerguard +two canonicalregressions actualnativeCLI **860 PASS**,
receipt ready-cli-validation-4n3v0ij9/pointer-complete-guard-receipt.json. Reviewer121
v2 pending. Homecurrent9106... actualcompile FAILED E0061 leadernumeric/wcallers
missingnewcompact_workerarg6932/6959. No testPASSforHome; optimizer118resumedv2.
Never treatprivategitapplyproofascompileverification. Mainheavyfreeafterfailure.
Do not combineoldHomecompilefailurewithpointer860PASSassingleGREENcandidate.

Integrated complete contextpointerrepair as **f1a902e1** after reviewer121v2
APPROVE0d2f2207... exactsourced56e2c31... andmain860PASS. Only native_view.rs
staged; preserveunknowncanonicaldecoder/userwebserverdiffs. Exactintegrated
targetfilterrerunpending; preintegrationcandidatebytesmatchedreview/testreceipt.

Renderer15v2 candidate49fb822f... usesabsoluteprivatepathsindiffheaders so
`git apply --check`failedBEFOREcompilation. Main now usesfiveexacthashchecked
candidatefiles (73c2.../EC1helper/82afview/f623entry/270edmain) onfresh eba783
archive in serializedshell sh_177ef9006001Eiv00CswL0uJ2Y. This is theFIRSTactual
dashboardcompileattempt, noPASSyet. Sourcefilesin15-renderfinish-v2/canonical/
crates/cli/src/. Independent14reviewactualv2 nowrunning, previousauditold9c0
notacceptance. Mainprivatecandidateviewincludesrouteguardbutnotnewcanonicaltests;
latercurrentf1a902compositionmustpreservethem.

Home118v2 actualsource215b7dd8... patchd1e431a5... addsmissingcompactworkerarg
toleadernumeric/w. Original131negativeagreesoldcompilefail; resumedv2exactreview.
No newHomecompilePASSyet. Hover108v2 nowexplicitvisible_close_rectpaintgate,
patches3bf6f171native_tabs/407d2da2entry; renewed122reviewstillneeded.
Kitty124sourcepreparedbutreportretainsPUArejectionandASCIIAltBackspacewhich
contradictpriorapprovedordinaryPUAfallback/realKitty57352/57353rule; independent
reviewneededbeforeheavy/acceptance. Do notintegratefromauthorprotocolsummary.
Arrow115sourcepatchheadersprivateabsoluteandbaseclaimincorrect; evaluator132
ses_e8811ad6bffepwbCuz91EWeBL2 reviewsactualapplyability/semantics independently.

Exact integratedf1a902e12839cec840b5d44e10c03036cabc1087 pointerfilter now **2 PASS**
withallcandidateoverlaysremoved; receipt pointer-integrated-receipt.json. Full
preintegrationexactbytesuite860PASS retained. Homev2source215b7dd8... now actual
fullnativeCLI **861 PASS** privatecompositionwithf1a902view; receipt home-v2-receipt.
Renewed131reviewpending. These are distinctsourceidentities; HomeNOTintegrated.

Dashboardv2actualcompiler **FAIL 5 E0609/E0560**: UsageReportmissingcoverage_gaps/
estimated_total_tokens despiteusages+initializer. Rawlogscontext-dashboard-v2-
compile-m6avdxds/test.stderr. Author15resumedv3fields/allconstructors+relativepatch.
No dashboardtestexecuted, no rendererGREEN. Heavy slot now FREE.

WRITE-SCOPE CONTAINMENT: canonicalnewdirtypermission files (run_owner,mcp_context,
todo,task_children/tests,native_view_dialogs,tui_entry,lib) match129handoff which
claimsimplementedinassignedrepositorydespiteexplicitprivate-onlygrant. Canonical
tools/executor+shell_toolnewdirtyalsoobservedwhile130active. Directed129/130/124
toSTOPALLcanonicalwrites, preserveuserwork(noreset/revert), returnactualpaths/
preimages/tools/provenanceprivate. These workers mustnotclaimnosourcewritesfrom
'noGitmutations'. No suchdiffstaged/integratedbyorchestrator.
Preservedforensic snapshot of13workingfilesandHEADpreimages+working.diff:
`shared-write-provenance-sw7j9n4j/manifest.json`. Userdirtylib/daemon_authpreexisted;
DO NOToverwrite/resetwholeroutefiles. Private119lib.user-beforehasprioruseragent
streamwork andcouldassistprecisepreimagecomparison afterworkerprovenancearrives.
UnconfirmeddecoderTabNext/Previous/Slotdirtydiffpreservedseparatelywithsamecopies.
Canonicaldirtytree is NOTreleaseattestedsource. All build/tests isolatedarchives.
Transport interruption recovery: last attempted patch call had empty arguments and
failed with provider WebSocket inbound queue overflow; no successful restoration
patch was applied. Canonical tools dirtydiff was inspected and matches130callback
changes. Worker130admitscanonicaltools+serverlibwrites; private130/worker.patch
568f963d... preservescandidate. Worker124admitscanonicaldecoderwrites; own124/
provenance.mdf4912e... recordsactual57352/57353unimplemented/PUArejectiongaps.
Do not restorewholelib; useragentstreamworkpreexists. Forensiccopies retained.

Dashboardv3actualcompilerFAIL E0373 inner .map closure borrowsownedline.styles
at338. Main applied ONE LINE privatecompilecopy `map(move |text| ...)`, no
canonicalrendererwrites. Privatecopy now distinctunreviewedcandidate; notyet
rerun/hashfreeze. Authorv3fa57... remainsimmutable/rejectedcompileartifact.
Homev2independent131APPROVE510bc7f4... exact215b... +main861PASS; integration
blockedbyunreviewedpermissionworkerwritesintui_entry. Preserve/identifyworker
preimageandpreparecleanindexratherthanaccidentallystagingsecuritycandidate.

Continuation: attributed124/129/130canonicalwrites now precisely removed using
scopedpatch hunks AFTERworkeradmissions+forensiccopies. Verified11workeronlypaths
byte-identicalHEAD; sharedserverlib byte-identical119savedUSERpreimage hash
a63ca89b68ed9c10fade2b0fb5a58c97a8f63989fcd31ec9fe294d96aaa7aa22.
No useragentstream/daemon_auth/web edits removed. No wholesaleGitreset/revert.
Private rejectedcandidatepatches/provenance preserved outsidecanonical.

Homev2canonicalpatchapplied viafunctions.patch produced EXACT215b7dd8... hash;
review131510bc7f4... +main861PASScheckedbeforeonlytui_entry.rs staged. Integrated
as **6ab59222**. Mainexactintegratedtui_entryfilter now running serializedshell
sh_177f9a3f80018gUBE27iVz2vzK, disposablearchivevalidation paths. Installedstill
unverified. Userbinaryunchanged. Do notclaimfullPhase1orUIparityfromthesechecks.

Dashboardv3+privateclosuremove compiles: **33 PASS 5 FAIL** of38native_context
tests, move-fix.stdout/receipt incontext-dashboard-v2-compile-m6avdxds. Fiveactual
semanticfailures: missingMessagescoloredspan; inventorydetailprovenanceunreachable;
projectionavailableabsent80x8; smallviewportselectedsummaryabsent; unavailable
contentdetailunreachable. Author15resumedprivatev4 preservingsemanticassertions,
estimated-onlyfallbackwithprovenancealsoexplicitfix. No dashboardintegration.

Incoming127catalogreportincorrectlyclaimsdoc/last_updated/open_weightsmissing;
canonicalcataloglibhasall+serdeflattenretainsmetadata. models.dev URL differs
frompinnedupstreamdefaultconfirmed; liveCDN/ETagclaimUNVERIFIED(noevidence).
134models-evidence ses_e88071d22ffe2Tx6S5GN2rJfWb taskedcorrectdurablesourceaudit.
Incoming17endpointworkerreturnedNOARTIFACT andwrongsourceclaims; upstreamTSsource
exists, actualbackendv3doesaddusage_report. 0testfilterdoesNOTprovetotalHTTPtest
coveragezero. Replacement18endpointactual ses_e8806ce70ffeYF7J1UlqF5EWjQ usesexact
backendprivatecandidatepaths realtestsrequired. Tool126claimedfunctions.patchbut
copiedunifieddiffandconflicting62ce/62cefhashes; verifyactualbytesbeforereliance.

Exact integrated6ab59222 Home/nativeentryfilter **95 PASS** (766filtered) receipt
ready-cli-validation-4n3v0ij9/home-integrated-receipt.json. Installedacceptancefalse.
Chat114v2appliescleanlyoncurrentHome+pointerbutactualtests **0 PASS 2 FAIL**:
longclippedprefixvisitorcapswidthso`cells>text_width`neverdetectsclipping; Unicode
fixturebyte2inside3byte界wronglyexpectsSome. ExistingSend/homecursortestlost#[test]
registrationinv2. Author114resumedv3naturalmeasure/clipping; independent135testowner
ses_e8804eb68ffe4r1TWkg8BKnD4e correctsfixturederivingactualrendergeometry, preserves
invalidboundary/longdraftassertions/restoresoldtestregistration. Noassertionweaken.
MainheavyFREE afterfailure, no otherworkerheavygrant.
136accountingcompositionreview ses_e8804a43cffem4MCMi1YBPkgWs reviews7ed71577lib
delta onactualpreexistinguserliba63ca89b... beforeapprovedbackendintegration.
122originalhoverreviewresumedv2actualpaintprojectione05ec80b/019fc0bd.
RouteusageUNKNOWN; active20roster notachieved/audited, sourcewritecontainmentand
repeatedcompletedno-artifactworkers reducedcapacity. Do notclaim20fromhistory.
Continueusefulindependentwork/replenishmentwhilekeepheavyserializedandonewriter.
MainSOLEintegrationwriter remains; lastproductSHAf1a902e1. Userbinaryoldstillnot
updated, daemon73389/4096notstopped, norealOAuthperformed. Negativeverdictsheld.

Latest: product HEAD `1c639d753c9c3fcfdc923c9540d60fab23b918a7` integrates
metadata/home cursor exact reviewed/tested97be7e92... source. Main's full native
CLI suite on this integrated product source: **858 PASS, 0 FAIL**. Raw logs and
receipt `ready-cli-validation-4n3v0ij9/metadata-integrated-cli-*`. Only native_view.rs
staged in this commit. Installed user binary NOT updated; full parity NOT claimed.

Context grid helper v4ec1c403e... actual independent rustc test run **11 PASS**,
after v2 had8PASS3FAIL arithmeticpanic and v3 had10PASS1FAIL deterministic tie.
Frozen tests retained through corrections. Receipt/rawlogs
`context-grid-v4-verification-e_5xi627/`. This is a tested helper, not live dashboard.
Original renderer still owns01 mutable production candidate and painter proposal;
independent13-gridreview `ses_e882911fbffelaWgKJ54QBmFy7` inspecting frozenv3,
will need finalv4 exact-hash review. Helper integration into renderer still pending.

Metadata v4 zero-context cursor hunk requires git apply --unidiff-zero in private
validation, then exact97be... hash asserted. First normal gitapply failure occurred
BEFORE tests; corrected attempt ran3PASS, then scoped integration and858PASS.

Markdown corrected6c61... compiles but actual37focusedtests **36 PASS, 1 FAIL**:
fenced-code preservation test joins rows without newline and compares multiline
expected; actual failure retained in markdown-corrected.stdout. Determine test
reconstruction vs lost-line semantic before modifying any assertion. Not integrated.

Additional useful replacements/latest work:
- Context backend v2 NEEDS_FIX9d56e728... sole remaining mismatch: memory injected
  but notrecognized at input0 emits Some0 while coverage says unreported. Pending
  author repair should useNone forunknown, then exactrereview. No capture integrated.
- Consumer originalnegative0/92 structuredpredicates -> author resumed08 actual
  dispatch, evaluator11notes64a229...; no fixturedefinitionacceptedasrun.
- Pointer e88cf570... independent reviewer `ses_e882a160fffe9IHVmD1urKIdL8`
  (12-pointerreview), actual native mouse handler included, source only.
- Shell preview c0748b8c... original reviewerAPPROVE77241cc6... isolatedprojection;
  actual ACK/painter caller wiring remains absent.
- Home repair88b4e62f... original reviewer `ses_e883ac366ffezx0lu3n2vkWO9O`
  renewed review. Detaching handle/drop receiver must not cancel daemon work.
- Vertical fixturea5d9... legitimate intendedbehavior but productmissingkind7
  hover/render_with_hover and orientationresizefallback. Runtime implementation
  `ses_e882acea0ffeX4Vv9qNktz02g6` (108-tabhoverresize), standaloneproposalonly.
- Horizontal v3 attempt had NOartifact; `/status` provides session id UUID fact.
  Own85 optimizer needs write-first actual helper/observed close, notanotherplan.
- Kitty coherentv3ae9138.../entry2dcc... combined9b034... -> reviewer
  `ses_e88279cddffecZR6Otc6v6gUh1` (110-decoderreview), preserveintegratedleader
  on latercomposition. No compileclaim fromvariantmention sourceaudit.
- Leader realbytefixture102 discoveredCSIArrowRight swallowed withnoInputEvent;
  cancellationoccursoneventpop so possible remainingstalechord. Diagnostician
  `ses_e882911f6ffern8MDyq8iVokN8` (109-leaderarrowdiagnosis). 855/858unitPASS
  doesnotprove this installedjourney. Do not callallleader behavioraccepted.
- Chatcursor actualnegative paddedmeasurement/modalguards: optimizer87 resumed,
  review105d309...; actualproposalhashc192... differsoldreported045..., reconcile.
- Keybind prose57bc... not implementation. Own86 author resumedactualRustmodule
  +canonicalruntimeproposal; no prose-to-product completion claims.
- Compaction v41diagnosis99noartifact/unconfirmed mixedstalesource -> newread-only
  `ses_e8826a2eaffehlczs6cu1yoRbw` (111-compactionexact), hashcurrentprivatefile
  first and writeinstrumentationproposal foractualsuppressbranch. Mainheavyfree
  after858PASS; future grant explicit, neverconcurrentCargo.
- Shell wiring97returnedowner-module-only; no runtimeintegration. Need tools
  executorverified-proof callback before lifecycleapprovaltransition; do not use
  proofpresenceassufficientauthority. Securitynegativeintegrationgapretained.

No release build/installed journey/user binary update/push during this segment.
Maintain usefulconcurrentassignments, ledgeractualcompletionstates, preserveuser
web/server changes anddaemon73389. Routecumulativeusage UNKNOWN. Do not re-count
completed sessions as active or infer quota remaining from successful calls.

Latest capacity replacements after final validation:
- Authorized `xkiro-gpt-6-luna` is an EXACT harness inventory identifier (notalias;
  no model override/fallback). First two dispatches now:
  `ses_e88256402ffewuAMWJq7Z37Jdl` replaces completed no-code108tab worker and
  owns108-tabhoverresize private base1c639copies/runtimepatch;
  `ses_e8825204effeeSUEd5ceTTHKEj` independent failingMarkdownline reconstruction
  diagnosis/testowner (112-markdownlinefailure). Tokenusage UNKNOWN, no newbudget
  inferred. Neither gets heavy execution permission.
- Contextaccounting author resumed02v3 unknownmemoryNone/minimalregression after
  originalnegative; all other v2 corrections preserved, no acceptance yet.
- Home originalreview APPROVED88b4e62f... a4d445db...; currentleader composition
  preparer `ses_e8824b62fffexg6d3CTfOekHyz` (113-homeready) usesbase1c639 and
  newcomposedhashrequiresrenewedreview. No oldapprovaltransfer.
- Gridhelper11PASS v4 exactec1c403e... can go to reviewer13/newrenderer integration
  after currentprivateauthorhandoff; don'toverwriteactiveauthorcopies.
Main serialized heavy slot FREE after858PASS and11gridPASS. Most urgent next
actualexecution: backendcapturecompilercheckafterv3review, correctedcontext
dashboard+grid integration checks, andHome composedfocusedtests. ReadyCLI tool
projection26PASS stillunintegrated; releasebuild/installedjourneysremainpending.

Chat cursor87 optimizer completed with NO v2 files; padded measurement/modal
blockers remain. Replenished with authorized exact xkiro-gpt-6-luna worker
`ses_e88238007ffel7KrMpn0CE2H2v` (114-chatcursor), exclusive new private directory,
base1c639, real source patch/tests required. No heavy execution grant. Preserve
fresh-frame cursor assertions; no product completion from this assignment.

Context memory v3 actualsource5180558f.../libf7ac6bad.../httpb9eae549.../
memory_http024e9da9... now frozen; original evaluator10 resumed exact composition
review of delta3a3cc9ee... with v2. Main serialized heavy slot NOW owns compiler/
focusedregression command in shell `sh_177dd407f001Dv5cEZLMcS8Baz`, disposable
archive1c639 plus fourexacthashedcandidatefiles; userdirtyserverfiles untouched.
No result yet; regression source alone remains UNEXECUTED until notification.

LeaderRight-arrow diagnosis109 confirmed recognized CSI C silently consumed so
leader remainsarmed. Replacement authorized xkiro worker
`ses_e88227209ffecIZJ9QtLfprOUd` (115-leaderarrowfix) owns newprivate base1c639
decoder/entry copies and minimal event/runtime patch/tests. Existing77candidate
underreview frozen; do not mutate it. No unknownprotocol blanketcancellation.
Independent review/realbyte installed execution stillrequired. UsageUNKNOWN.

Context accounting v3 executed validation: exact four source hashes fromv3manifest
compile successfully in fresharchive1c639. New unknown-memory regression **1 PASS**;
full context_accounting filter **4 PASS**. context_http filter executed **0 tests**,
so it is NOT endpoint coverage/PASS. Logs/receipts in
`context-accounting-v3-validation-i7kh754x/`. Broader candidate server --lib run
now mainserializedshell `sh_177de50630017PFs6CRcXSniKE`, result pending.
Reviewer10 independently evaluating exactv3 composition; no serverintegration yet.
Candidate server --lib completed **294 PASS, 0 FAIL** on exactv3 source hashes;
receipt `context-accounting-v3-validation-i7kh754x/server-lib-receipt.json`.
Heavy slot released. These are privatecandidate tests, NOT integrated/dashboard/
installed acceptance; originalindependentreview stillrequired.

Markdown independent testowner112 determined linefailure is badrowreconstruction,
not demonstratedrendererloss. Test-only correction candidateebb7a4e... now main
executed **37 native_view_chat PASS**, receipt markdown-row-repair-receipt.json in
ready-cli-validation-4n3v0ij9. Originalevaluator28 resumedv6 exacttestintent review;
no weakeningapproveduntilreview, no integrationyet.

Kitty combinedv3 review110NEEDS_FIX losesintegratedleaderpendingclear. Author77
resumednewbase1c639 composition preservingfa708leaderbehavior/newmetadata97be.
Originalnegativebc09f5c1... retained, decodercandidate notready.

Contextpointer e88cf570... reviewer12APPROVE8ad3fb5e...; mainnowserialized
validation shell `sh_177dff9ef001e2oz9S0C6Neidc` appliesapproveddiff onto1c639
privatearchive preservingmetadata/leader and runsnativeCLI suite. Pointergeometry/
realclickjourney stillunproven; compile/testresultpending. No other heavy grant.

- Current product HEAD now `2aafbfbf` (leader dispatch/cancellation), exact reviewed
  source hash fa708ed3ce3d1968034e5957d3ff846e971e2de52f8aca41895a25c730b7840b.
  Only tui_entry.rs committed. Private candidate compiled; two shared close-result
  tests PASS. Installed real-byte leader journey remains UNEXECUTED.
- Main resumed the serialized heavy slot after compaction worker completed with
  v5 unfinished and BOTH runtime tests failing provider-boundary timeout355.
  Compaction diagnosis read-only worker `ses_e883554daffeZIdNDpaQyGKqgz` (v41)
  owns replenishment99-compactiondiagnosis. No current v5 patch accepted.
- Actual validation archive `ready-cli-validation-4n3v0ij9/` at ee9b92a1 with
  approved candidate overlays: Markdown808855... FAILED COMPILE E0308 at File
  match arm1214 (push_wrapped now bool). No test PASS attributed to this candidate.
  Repair106-markdowncompile candidate6c61dbf8... from original author; original
  reviewer `ses_e8858daf1ffeMcw3IglEkcob6z` renewing exact review.
- Removing Markdown overlay allowed independent actual validation:
  native_chat_projection **26 PASS** on62ce0f30...;
  composer_metadata_regression_tests **1 PASS, 2 FAIL** on4a093230...;
  render_fixture_tests **1 PASS** (actual six Home/Chat frame outputs);
  shared_close_result **2 PASS**. Exit0 shell is not all-tests-PASS: per-command
  exit codes/logs retained in isolated-ready-receipt.json.
- Metadata actual failures contradict static arithmetic: width43 actual model
  `modelmodelmodel…`, expected`modelmodelm…`; Home cursor actual87,19 expected19,19.
  Original optimizer resumed40-metadatafix; do not integrate statically approved
  candidate until corrected/reviewed/verified. Preserve semantic assertions.
- Main full exact-integrated CLI validation on2aafbfbf now background shell
  sh_177d304ab001MVqNEpo50EgWJL; all candidate overlays removed from private archive.
  No other worker has Cargo/PTY grant. Installed acceptance remains false.
- Context accounting eaa70e9d... review NEEDS_FIX missingcomma608/MCP capture
  before refreshed tools/compaction gap/memory source loss. Author resumed02 v2.
  Reviewer `ses_e8834a4e0ffeYYsRXWhHik9V5C` notes saved10-accountingreview.
- Context frozen renderer9c0... NEEDS_FIX compile/unknown-grid/overflow/narrow.
  Original author resumed01; separate09 helper author fixing its admitted partial
  marker ordering invariant. Neither mutable artifact is ready for integration.
- Context pointer e88cf570... now actual mouse handler included and private
  applycheck PASS; independent review still pending. No actual context click PASS.
- Context golden consumer798e26ca... actual Rust tests authored, no execution;
  evaluator `ses_e882db7b3ffe7sdnx4qBnzfk3j` (11-consumerreview) reviews API/assertions.
- Raw six Rust frames being extracted from executed log by
  `ses_e882ddd15ffekFVC0e10nzYT1b` (107-rustframes); approved91 comparator is
  text-cell-only. Color parity UNVERIFIED; no Rust-frame provenance invented.
- Cursor proposal045759c0... reviewer `ses_e882fff26ffeN6gLCnCmvftl84`
  (105-cursorreview), source-only geometry/security review, no fixture weakening.
- Permission second reviewer retracted unsafe-exposure objection: live callers
  pass absent/empty save so persistent allow hidden; eligible-save parity remains
  incomplete. Existing exact-source security approval62627f94... still scoped.
- Permission realEscape fixture93097ffb... APPROVED original reviewer5c69908e...;
  no PTY execution. Denial extension preserved. Shell previewc0748b8c... corrected
  width/C1 filtering but not yet rereviewed/wired/tested.
- Horizontal bc1d2b00... original evaluator resumed76-v2; vertical a5d9d7a7...
  original evaluator resumed103-v2 for actual middleSGR/rightneighbor/fallback.
- Kitty decoder601d7d84... author resumed77 actual exhaustive native-loop wiring.
  Keybind57bc... was prose despite .diff extension; NEEDS_FIX remains until actual
  applicable source patch exists. No prose accepted as implementation.
Route telemetry UNKNOWN. Recent route calls all allowlisted; no provider fallback.
Replenishment lag occurred during checkpoint/read/serialized validation recovery;
do not claim an audited20-active roster from historical launches. Completed workers
are excluded; continue useful independent replacements, bounded writes and one
heavy slot. Preserve unknown user web/server edits and daemon73389/4096.

User supplied Claude Code screenshot/text and explicitly wants that `/context`.
Read `docs/CONTEXT_USAGE_UI_REQUIREMENTS.md`. UI/private accounting/pointer work
is in progress; inventory-only display is not the requested completion. Primary:
`ses_e884a754affe67GhqY705ThGFn` renderer/painter proposal;
`ses_e884a7545ffenjzK3cSw9agMtH` actualrequestcapture/usage_report;
`ses_e883ed3e3ffeLI48tWqLyqDqaa` missing context pointer hitbox/handler proposal;
`ses_e883b519fffeZ8jwTxEaIf77Z1` frozenrenderer9c0eade9... review.
Context fixturev3 SHA677fac0a... under context-user-03-acceptance, original
evaluator `ses_e8846df84ffe9tLSREPE846AL0` reviewing contract only.

Maintain20+useful assignments/replenish notifications. All recent dispatches on
allowed Luna or <=6v41; no paused GLM/TokenHarbor/combined routes used.
HEAVYslot: compaction optimizer `ses_e88bd254fffeg0P1UZYgSytZK9` owns v5
race-regression compile/tests. Main must not run concurrent Cargo/PTY until slot
released. v4 NEEDS_FIX independent snapshot-read race remains blocking.
After release run focused CLI validation of APPROVED Markdown808855fe... and
toolprojection62ce0f30... candidates, then integrate readyexactbytes promptly.
Render capturefixture1232faf2... APPROVED source/wiring; validationoverlay prep
`ses_e883c7028ffeRPAEOi5oQeDlIx`. Actual6Rustgrids still not captured.

## Route allowance snapshot

The complete allowed routes and stop rules are in `AGENTS.md`. Aggregate consumed
tokens and remaining balances are **UNKNOWN**: no reliable cumulative telemetry
has been recorded. Do not infer remaining quota from a successful call or reset
budgets on compaction. `vyce-deepseek-v41` has an eight-concurrent-assignment cap.
Resolve the user's `xkiro-gpt-6-luna` spelling against the actual harness inventory
before dispatch. No unlisted fallback is authorized. Maintain a session/route
usage and error log here as new telemetry becomes available.

## Verified release and installed journeys

Release source SHA: `b24eb60918c41ae215dedce996ef27b8a96968f3` (older than HEAD).
Evidence root:
`/Users/mymac/Projects/opencode-rk/.git/recovery-evidence/b24eb60918c41ae215dedce996ef27b8a96968f3/native-release/`.

- Earlier exact-release focused validation: 890 tests passed; release build,
  package and disposable installer checks passed. This is not final-SHA acceptance.
- Correct canonical installed scripts were rerun serially; results under
  `installed-spine-short-runtime/`: provider setup PASS, stream/tool PASS,
  second-client FAIL (`PTY EOF before marker b'stream fixture openai'`).
- Earlier runner used wrong-tree evidence scripts; retain those logs but do not
  treat them as canonical-script acceptance. A separate first canonical attempt
  failed before launch because TMPDIR made the socket path exceed 100 bytes.
- Short-runtime run used the macOS `.../T` parent, not an extra nested directory.
- Do not install to the real user prefix or push as a verified final release yet.

## Context repair: proven status bug and candidate

User requirement: context must be accurate; clicking it and `/context` open the
same session-bound detailed inspector. Requirements are recorded in
`docs/upcoming-features/context-inspection-parity.md`.

Confirmed root cause: `StatusView::set_usage` used `window.map(...)`, discarding
measured input/output when the selected model's capacity was absent. Repair keeps
the measured total with an optional limit, showing capacity as unreported. Cache
and reasoning are inclusive transport subsets in this path; do not blindly add
upstream's decomposed fields again without tracing normalization.

Private verification tree:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/context-red-green-d7y4lapv/`.

- Independent coordinator regression compiled and failed for the exact bug:
  `red-bin.stdout`; original `--lib` invocation was invalid (CLI is a binary).
- Status repair passed that same regression, all 34 status tests, and all 838
  CLI tests (`status-only-cli.stdout`). Disposable HOME, offline locked Cargo,
  Rust 1.98.1, one build job/test thread; real native library linked.
- Exact status-only package:
  `.../T/opencode/context-status-reviewed-package-155do1nh/candidate.patch`.
  Patch SHA256 `b4e30adaa07fac5ba289b39d2a1754985db28d75b1adbd394447a1ba023af14f`.
  Resulting status file SHA256
  `43e309db7c63b6a8708bf9e45081ffd8a4170919da1a4dd2c02b5f0262c0bbd3`.
- Reviewer `ses_e88e5d841ffes0OS7vHhKj793o` APPROVED this exact status-only
  package and existing-assertion changes. Integrated as
  `14a4fb44bb2fcf8660b28ffef6e77abac5489b30` after the operating-rule commit.
  Candidate preverification passed 838 CLI tests. Exact integrated release build
  and installed acceptance still required; receipt is next to the package.
- Integration preflight after the documentation commit found an unexpected
  canonical status-file edit, SHA256
  `103e49eda891ad95a122df4c1b1a5ee70a70e685f1cc48018be7a75f592f0017`.
  It adds cache/reasoning to inclusive totals and changes the 150 assertion to
  200; it is NOT the reviewed candidate. No approved patch was applied, no
  status code committed, and the unexpected working file was left untouched.
  Exact bytes/diff were preserved at
  `.../T/opencode/unexpected-status-edit-preserved-kll4yq2i/`.
  Provenance/stop-write requests went to research sessions
  `ses_e89031db5ffehnSPru0cN5qDUZ` and `ses_e88f61dccffeqIbc4Ma0YUzlLv`.
  Session `ses_e88f61dccffeqIbc4Ma0YUzlLv` subsequently completed and identified
  its exact canonical candidate SHA (despite contradictory prose claiming no
  edits); the other session denied writing. The completed worker's identified
  delta was preserved, hash-checked and reversed, then the independently approved
  package applied and committed. No unrelated work was touched. Future assignments
  must reiterate read-only grants and absolute private artifact paths.

## Detailed inspector candidate and independent diagnosis

Candidate adds truthful optional byte/estimated counts, provenance, withheld-content
placeholders and page-inventory category rows. It is not a measured whole-request
token split or proven exact Claude Code visual clone. No local Claude Code source
was found. Upstream sidebar usage is distinct from the requested `/context` extension.

- v1 fabricated category zero totals for unknown items; rejected.
- v2 corrected unknown/mixed counts but five rendering assertions failed because
  one long breakdown row hid category data. No integration.
- v3 split categories into short rows. A test helper initially failed to compile
  because scrolling needed `&mut ContextView`; corrected only in private tree.
- The resulting run passed 22/23 tests. Remaining failure was generic
  `!seen.contains("3 items")`, colliding with truthful page header
  `Context · 3 items · projection available`.
- Independent reviewer `ses_e88ea7c96ffeXWoM1LdIeaU8aB` confirmed this as a
  mechanical assertion-scoping defect, approved product code provisionally, and
  required a category-scoped negative before acceptance.
- Coordinator corrected it to reject `3 items` only on a `conversation:` row.
  Final rerun and exact aggregate re-review are still required.
- Exact assembled aggregate candidate (three files, includes status regression):
  `.../T/opencode/context-inspector-final-review-1ykpznlf/candidate.patch`.
  SHA256 `af4f1d5c2a51f48c137b1651f5b64bbeabca7c94fae502ea01e69860a2cdae7a`.
  Product `native_context.rs` SHA256
  `f5adeda00904178c42b586bc1b7324c5248c137d1babcd950a1d8bdeac1b2aed`;
  test SHA256 `40d84932925c3ecac764bb8e920a24e9e930bddc507b40dc86bdec7447768b7b`.
  Do not mistake status-only approval for approval of this larger patch.

## Rendered parity evidence and current dependency blocker

Pixel-perfect labels, coordinates and colors remain UNPROVEN. Multiple source
audits used the wrong pin or inferred color/position claims from selected text
excerpts; require verified sources and actual rendered frames before acting.

- Actual full-app probe v2:
  `.../T/opencode/rk-visual-probe-20261026-v2/home-chat-capture.test.tsx`.
  SHA256 `56d51ea1a63a336438a71e2b48b5054db0d66a2899ef41b67e0760444e1d7d12`.
  Mounts real upstream app using `createAppFixture`; waits for bounded `ready`,
  Home/chat markers and visual idle; captures grids/spans at 120x40, 106x40, 105x40.
- Disposable source and logs:
  `.../T/opencode/upstream-render-capture-roo47m1l/`.
- First run failed before mount: source requires OpenTUI 0.6.8, pre-existing
  upstream node_modules resolved 0.4.5, missing `NativeError`. No frames produced.
- Known disposable dependency symlinks were removed without touching upstream;
  isolated `bun install --frozen-lockfile --ignore-scripts` completed with exit 0
  (shell `sh_177214fe3001eo7zjH9R1QG01e`). Inspect the install receipt,
  then retry probe with disposable HOME. Do not poll or start concurrent heavy
  validation. Do not silently alter dependencies or swallow readiness failures.

## Other pending packages and review cautions

- Second-client EOF diagnosis: `ses_e88ea7c8cffekjUIJZtZkr6uPJ`.
  Discovery audit claims about macOS PID liveness/config roots are unverified;
  correlate actual source and launch stderr before proposing a fix.
- Horizontal fixture repair: `ses_e89031dbaffe6BM26XShd7ipi9`; reviewer
  `ses_e88eb2ed1ffe6ExuTgL0v6GS6U` rejected prompt-label registry because tabs
  render daemon titles (`Chat`), not prompts, and no real UUID binding existed.
- Vertical fixture already committed in `52a89...`, self-check PASS, installed
  journey UNEXECUTED. `ses_e88fed35fffeX6HLf9r1akKOtL` owns private repair for
  catalog display-name readiness and the same false prompt-as-title assumption.
- Home audit corrections: `ses_e88f83ce1ffeTeqkUDWUvQJTEM`; capture author:
  `ses_e88f83cd9ffe1o6ItW6EsgujZn`. Require exact component/paint-path evidence.
- Compaction retained-tail regression: `ses_e89012d76ffe21U8NIArlnrfXX`;
  independent strategy review `ses_e88e8f6ecffeu84DgOMbalOC4l` rejected earlier
  standalone tests (private helper visibility, dropped env guard, waiting for a
  background checkpoint that is committed only on foreground reuse, overspecified
  boundary assertion). Need real preserved-history regression before product fix.
- PTY persistent ownership, keyboard tabs, browser/Copilot OAuth, full catalog,
  subagent inspection and whole-product coverage remain unfinished/unproven.
  Scoped device-auth coverage was 91.74% lines, not whole-product or 100%.

## Preservation and dispatch follow-up

- Untracked `.private-tmp/`, decoder `.red_snapshot.rs`, `tests/e2e/T/` include
  worker mistakes; preserve bytes before any cleanup and verify writers stopped.
  Preserve unrelated `integration/` and `manifest_output.txt` user work.
- Existing assignments predate the new scratchpad requirement. On their next
  substantive dispatch require explicit role/goal/success criteria first, then
  `docs/AGENT_WORKER_PRACTICES.md`, and a unique scratchpad path/hash.
- Session IDs above identify prior assignments, not a verified live-worker count.
  Rebuild the active assignment roster from harness state/notifications; maintain
  20+ useful allowed assignments with the per-route concurrency cap and one heavy
  slot. Do not describe completed agents as active or poll pending agents.

## Immediate next actions

1. Finish and preserve the requested agent instructions/scratchpad documentation.
2. Receive isolated dependency-install result; retry real upstream capture.
3. Integrate the approved status-only package promptly, or the combined inspector
   only after its corrected tests and exact independent review pass; do not stall
   the status repair on unrelated blockers.
4. Rerun corrected context suite and affected CLI regressions; preserve receipts.
5. Repair second-client failure and tab fixture identities; run exact-release
   installed journeys after final integration. Rebuild for the actual final SHA.
6. Complete verified installation and fast-forward push only after release checks.

## New role-first dispatch roster — 2026-10-26

Twenty new background assignments were launched on the allowed unlimited
`9router-vyce-deepseek-v4.1-free-combined` route after the rules were committed.
Each prompt assigns role, goal and success criteria first, then worker practices,
explicit read-only/private grants, source pins, resource limits and a unique
research scratchpad path/hash requirement. These are launch receipts, not a claim
that every assignment remains active after this snapshot; update on completion.
No route usage telemetry was returned; consumed/remaining tokens remain UNKNOWN.

| Session | Assignment |
| --- | --- |
| `ses_e88d69ce8ffeZCusa8cOLB0F2I` | Final context aggregate review |
| `ses_e88d69ce2ffeQjDPvtUDUFQuh6` | Wire-to-UI token normalization evidence |
| `ses_e88d69cd6ffeJHwwPLF1hy3irJ` | Private context installed-journey author |
| `ses_e88d69cd4ffeXQ7i0yUzX5nXew` | Operating-instructions audit |
| `ses_e88d69c4affelIqL0fWyzjnAYe` | Model output limit propagation |
| `ses_e88d69c49ffelbqhgSyI6SudfU` | Credential/catalog availability contract |
| `ses_e88d69c44ffevqSB0bay7HDBvD` | Browser OAuth parity package |
| `ses_e88d69c3effefvtxyLUf4G1uMv` | Copilot entitlement/transport contract |
| `ses_e88d69c3dffeP2Guqpqw6bOIE1` | Real PTY cancellation regression review |
| `ses_e88d69c37ffejbFbCJF1HONrRe` | Persistent PTY ownership contract |
| `ses_e88d56019ffeYYipe9Aw99jevG` | Private keyboard tab decoder candidate |
| `ses_e88d5600fffepyWKGmG4cZKga7` | Private leader tab dispatch candidate |
| `ses_e88d56006ffewtocB1Wmf4VBr1` | Child inspection behavior review |
| `ses_e88d56000ffeqoLH4ZPxXG05qe` | Executable honest coverage manifest |
| `ses_e88d55f84ffe61W2pion4IU1Qz` | Release/package provenance prep |
| `ses_e88d55f7dffefeGDtU1f8fLfQu` | Worker artifact preservation inventory |
| `ses_e88d55f7dffdly4XpexGhsHhLE` | Global CLI settings parity |
| `ses_e88d55f6affe1nSMUg6fSHhOTM` | Last-tab Home direct-send safety |
| `ses_e88d55f68ffeS42RTNZ4kig7v2` | Third compaction regression strategy review |
| `ses_e88d55f68ffdrB0eOsXmq6vHmD` | Current device-auth edge fixture review |

Heavy slot: upstream capture retry with correct locked dependencies launched as
`sh_1772acd18001sScpqMRKhltHgg`; logs/receipt `locked.*` under the disposable
upstream capture root above. It completed: **4 PASS / 2 FAIL**, exit 1. Home at
120x40 and chat at all three sizes captured actual frames; Home 106/105 timed out
because the fixture demanded one specific rotating example while the rendered
placeholder showed another valid example. Preserve failures; repair readiness
against the actual placeholder contract before retrying only the failing cases.
The captured upstream footer visibly includes `ctrl+p commands`, refuting the
earlier source-only audit that called it a fabricated Rust hint. No Rust visual
comparison or pixel parity is established. Heavy slot is released. Corrected
inspector tests and exact integrated-SHA CLI checks are next in that serial slot.

Operating-instructions audit completed APPROVE with a minor authority-list wording
clarification, now applied. Durable rationale and original scratchpad SHA256 are
in `docs/worklogs/research/2026-10-26-agent-operating-rules-review.md`. The worker
wrote inside canonical `.private-tmp/` despite its read-only grant; preserve the
original and require a genuinely external absolute scratchpad on reassignment.

## Subsequent verified findings and availability changes

- Corrected inspector tests now PASS: 23 context tests and all 847 CLI tests in
  `context-red-green-d7y4lapv/final-{context,cli}.stdout`; final receipt records
  exact source hashes. Original evaluator `ses_e88ea7c96ffeXWoM1LdIeaU8aB`
  received the final aggregate and APPROVED exact SHAaf4f1d5c... . The two detail
  files were integrated as `27722646f55ba59276d1308e7b2e55eaff031b04` (status was
  already integrated). A fresh exact-27722646 source archive with a fresh Cargo
  target passed **847/847 CLI tests**; raw logs and receipt:
  `.../T/opencode/context-exact-integrated-jjhruynv/`, plus the shared Git evidence
  receipt at `recovery-evidence/27722646f55ba59276d1308e7b2e55eaff031b04/context-integrated/`.
  Installed click and `/context` journeys still PENDING; do not repeat a reviewer's
  claim that preintegration candidate logs were exact integrated verification.
- Full normalization trace confirms inclusive Rust input/output must NOT have
  cache/reasoning added again. Upstream normalizes before session summation:
  `packages/ai/src/protocols/open-responses.ts::mapUsage` and
  `packages/core/src/session/usage.ts::tokens`. For input=100, cache=40,
  output=20, reasoning=5, upstream decomposes 60+40+15+5=120; Rust uses
  100+20=120. The unreviewed 200-total worker proposal was incorrect.
  Research note: `.../T/opencode/research-token-normalization/scratchpad.md`,
  SHA256 `4c5903f633dd66bffda5342986e92db7040cf342a6d1cc60ccd6aaa20e029cda`.
  Missing Rust cache-write detail is a separate gap, not a reason to inflate totals.
- Upstream Home retry accepted the actual rotating-placeholder contract rather
  than one particular example. `home-retry-receipt.json` exit 0; all six Home/chat
  viewport captures are now available across `frames-locked/` and
  `frames-home-retry/`. These are reference captures, not a Rust differential.
  All 12 raw files, hashes, label rows and run receipts were also preserved under
  the shared Git directory at
  `recovery-evidence/upstream-450e7b92412c780a7b5a75226bb8d49aaca24e22/rendered-home-chat/`.
  Repeated source-only claim that `ctrl+p commands` is absent is refuted by the
  actual upstream captured footer; do not remove it based on literal grep.
- Combined DeepSeek route returned `no eligible account` for sessions
  `ses_e88d55f6affe1nSMUg6fSHhOTM`, `ses_e88d69c44ffevqSB0bay7HDBvD`,
  `ses_e88d69c4affelIqL0fWyzjnAYe`. Pause NEW dispatches on that route; do not
  infer token exhaustion or balances. Existing workers may still return results.
  Replacements: GLM `ses_e88cf7e8cffeQQupeK9RWPjqgO` (Home send safety),
  Token Harbor `ses_e88cf7e88ffeq5jHM32xSWbaIM` (browser OAuth) and
  `ses_e88cf7e86ffemIND4Wyk4N2s7v` (output limits). GLM cumulative use UNKNOWN;
  stop dispatching it on any error as required by its budget rule.
- Horizontal F1v2 SHA6661c295... was independently rejected again by
  `ses_e88eb2ed1ffe6ExuTgL0v6GS6U`. Merely renaming a prompt-label registry
  does not fix actual `Chat` titles. Strategy note:
  `.../T/opencode/strategy-tabs-identity/TAB_IDENTITY_STRATEGY.md`, SHA256
  `15924b1a825573aa0e05b941270d9b8dbe2f4d9aa5899cc693584c0598e40ca2`.
  Next repair must use bounded rendered title occurrences checked against the
  authenticated UUID ledger, or genuinely created/renamed unique titles with
  verified propagation. No fixture acceptance from self-check/assertion counts.
- Original edge-auth negative reviewer approved SHA3a4d55... boundedness, but
  coordinator then found a missed EOF defect: `.expect(...)` on `read()` rejects
  errors, not `Ok(0)`, so the helper spins until timeout rather than failing EOF
  immediately. No test execution/integration yet. Token Harbor private author
  `ses_e88cea7e7ffeL1e1uHVpVJqm0U` owns explicit `n > 0` correction in a NEW
  external copy; original reviewer must evaluate that new hash again.
- Confirmed actual source defect: `daemon_client.rs::pid_alive` and
  `discover_presence` both use `/proc`, invalid on macOS. Server's existing safe
  rustix `kill(pid, 0)` implementation provides the repair pattern. Token Harbor
  author `ses_e88cde434ffe1Tk103SMKsVVrb` owns only a private daemon_client.rs
  candidate plus regression. No user daemon touched; do not call this the proven
  sole cause of the second-client EOF until the installed journey is rerun.
- Packaging audit `ses_e88d55f84ffe61W2pion4IU1Qz` reconciled old b24 receipt
  hashes, but its rebuild trigger list was incomplete: ANY linked product source
  change requires a new binary/release provenance for the new SHA, not only build
  scripts/lock/installer changes. Old b24 binary/archive remain evidence for b24;
  never relabel them as a newer verified release. Audit note:
  `.../T/opencode/auditor-b24-scratch/RELEASE_AUDIT_b24.md`, SHA256
  `10a925c2d9c6b2d1d15b06cab8bd7296fafe62636c44c53da419bb0528775e81`.
- Vertical repair author claimed `.../T/opencode/vertical-tabs-repair-v2/` but
  coordinator read confirmed that directory DOES NOT EXIST. Treat the handoff as
  INCOMPLETE, not an implementation. New Token Harbor owner
  `ses_e88ca554affeVqwp0ljN0bmuXS` owns real private horizontal/vertical fixture
  artifacts using the independent identity strategy and correct model labels.
- PTY reviewer `ses_e88d69c3dffeP2Guqpqw6bOIE1` approved the cancellation-test
  idea but reported the fix diff malformed. Do not apply a fragment or blindly
  change cleanup to return every existing state: review successful/running/error
  retry semantics and run a compiling RED/GREEN on a real candidate first.
- Edge-auth build-feature concern is now resolved by ACTUAL COMPILATION, not
  partial `cargo tree` output: exact test SHA3a4d55... compiled with Rust1.98.1,
  locked/offline `cargo test -p opencode-rk-providers --test
  openai_device_auth_edge_cases --no-run`, exit 0. Private source/receipt:
  `.../T/opencode/device-edge-compile-proof-ssxngo_a/`. No tests executed and
  no dependency changes; explicit EOF `n > 0` correction/re-review still pending.
- GLM Home audit `ses_e88cf7e8cffeQQupeK9RWPjqgO` refuted stale send for
  EMPTY Enter after last-tab `/exit`: draft is cleared and empty submit gate blocks
  the turn. It separately traced a real adjacent hazard: typing a NEW draft and
  submitting can use retained `live`/selected state and the closed tab's session.
  Do not report the empty-Enter result as proof that new-prompt routing is safe.
  Same GLM session reassigned to an actual installed new-prompt regression, private
  Python test only; shared `tui_entry.rs` implementation remains serialized.
  Research scratchpad SHA256
  `87e3f65b12bdddfd285c6b01324ddb19533740dafdac79d8bb8d5a9ec80be7da` at
  `.../T/opencode/review-stale-send-exit-lasttab-home-enter-2026-10-26.md`.
  GLM route working, no errors; cumulative tokens still UNKNOWN.
- Global CLI config audit reported that keybind overrides/leader timeout are not
  loaded (only tab layout parsed). Needs source validation before implementation;
  its returned scratchpad path points to a different `recovery-20261006` tree,
  so do not use that path as canonical evidence or claim a delivered artifact.
- Preservation inventory is available at
  `.../T/opencode/preservation-audit-scratchpad.md`, SHA256
  `5bf677988139e2f3d3d61cbbdbd0966d60569a9dd7d68d5995278241df8581e6`.
  This inventory is not authorization to delete user files. Its ETX-to-`/quit`
  copies remain REJECTED proposals, not valid test corrections; retain original
  Ctrl-C terminal-restoration assertions and do not integrate those copies.

## Latest handoffs and ownership — 2026-10-26

- Token Harbor free route `9router-th-dsv41-flash-free` is EXHAUSTED. Provider
  HTTP429 explicitly reports the next rolling period starts **2026-11-01 at
  08:15 UTC**. Failed sessions include EOF correction
  `ses_e88cea7e7ffeL1e1uHVpVJqm0U` and output-limit analysis
  `ses_e88cf7e86ffemIND4Wyk4N2s7v`. Stop ALL new dispatches on this route
  until renewed allowance is established; no paid-model switch authorized.
  Token counts UNKNOWN; exhaustion is provider-reported, not an inferred balance.
  Output-limit analysis remains incomplete. Coordinator prepared EOF correction
  privately at `.../T/opencode/device-edge-eof-corrected-lzghckq0/`, adding
  explicit `assert!(n > 0)` before extending the buffer. Original fixture
  preserved; corrected copy awaits new review and verification. No Cargo run
  while compaction worker owns the heavy slot.
- Further Token Harbor exhaustion failures: browser OAuth
  `ses_e88cf7e88ffeq5jHM32xSWbaIM` and macOS PID repair
  `ses_e88cde434ffe1Tk103SMKsVVrb`. These failed assignments provide no completed
  artifact or verification. Reassigned on allowed `vyce-deepseek-v41`:
  browser OAuth researcher `ses_e88b4ff51ffexsxIO3C4JVdV47`, macOS PID private
  implementer `ses_e88b4b383ffe5cFp62vAOnpQMe`. Same route also has EOF fixture
  evaluator `ses_e88b92804ffeagmejD1Ya47dQS`. These three launch receipts stay
  below the eight-concurrent cap; no reported usage telemetry, aggregate UNKNOWN.
  No new Token Harbor assignments or paid fallback. MAC PID candidate must remain
  private/unexecuted until the serialized heavy slot is available.
- Coverage manifest verification `ses_e88c8a5d8ffeW5RdW2srvmx10Q` also failed
  on the exhausted Token Harbor route. Coverage manifest remains UNVERIFIED;
  no whole-workspace percentage or executable-manifest acceptance is established.
  Deferred behind installed Phase-1 blockers rather than retrying the exhausted
  route. Provider's parenthetical retry delay is not renewed free allowance;
  explicit rolling reset remains 2026-11-01 08:15 UTC.
- Browser OAuth replacement `ses_e88b4ff51ffexsxIO3C4JVdV47` delivered upstream
  research under `.../T/opencode/browser-oauth-v41-recovery-20261026/`.
  Coordinator verified actual SHA256: scratchpad.md
  `433d2619fa1dbfb32890969ea0fe3aa9bc52ec2ff5ed3a72d501c5d9a9ddf99f`,
  report.md `e3a0ddecea2d4c20cae0628447ee7027ca033f5e58cb0549df5dc7322bdd8246`.
  Research only: no Rust source inspection, implementation or tests. The report
  combines OpenAI/ChatGPT/MCP and contains questionable PKCE/refresh and persisted
  monotonic-expiry generalizations. Same worker reassigned to a narrow OpenAI
  browser-login Rust gap addendum, checking existing capabilities/credential
  wiring and correcting those claims against actual sources. Do not advertise
  browser login as supported based on this research delivery.
- Ledger-backed tab fixture author `ses_e88ca554affeVqwp0ljN0bmuXS` failed
  with Token Harbor `free_tier_limit_reached` (same Nov1 08:15UTC reset).
  Horizontal/vertical fixture repairs remain incomplete; no artifact accepted.
  Reassigned to allowed v41 private author `ses_e88ab51afffeVRtSKcgIyb6Mu2`
  with explicit external directory, actual title/UUID identity requirements and
  separate open-tab vs persistent-session ledgers. No Token Harbor retry/paid
  fallback. New author has no execution permission while compaction holds heavy
  slot. V41 launch roster now includes EOF review, OAuth gap, PID repair and
  tab-fixture repair (four known assignments, below eight-concurrent cap).
- MacOS PID replacement returned real private artifacts in
  `.../T/opencode/macos-pid-v41-recovery-20261026/`, but handoff is INCOMPLETE:
  draft tests, no compilation, CLI apply check not executed; server public-helper
  proposal exceeds original client-only grant. Coordinator inspected draft and
  reassigned same worker to generate actual client-only rustix implementation
  plus valid test-only and production diffs. Include actual discover_presence
  regression, not solely validate_descriptor, to catch duplicate inline probe.
  No server visibility change authorized; existing artifacts preserved. No
  verification or installed EOF resolution is established by this handoff.
- EOF evaluator `ses_e88b92804ffeagmejD1Ya47dQS` returned wrong-file findings:
  mistyped external path (omitted `y` in `dj81nc_j2yq2...`), then substituted
  canonical `openai_device_auth.rs` for separate private edge-case fixture. Its
  claims that artifact never existed/already integrated are FALSE. Coordinator
  reverified actual target file exists and SHA256
  `f3ea4ccfe30ff57df193048cfed7632a4dcb975593625696e9647c67da1e8ff5`.
  Same evaluator reassigned with exact path and fail-closed BLOCKED instruction
  on access failure. Existing canonical helper's 5s read timeout is irrelevant to
  edge fixture's nested 10s aggregate/5s per-read limits. No approval received.
- Luna compaction candidate delivered actual `source/evidence/candidate.patch`,
  command manifest and `final-correct-target.log` under its private root. Worker
  reports compiled pre-fix provider-continuation failure, then focused 1/1 PASS.
  Production guard reuses held summary only when captured durable head equals
  its retained assistant anchor; otherwise cancels held work and recomputes.
  Storage head identity check remains unchanged. Coordinator inspected patch:
  it REPLACES existing valid-reuse scenario and changes summary count 1->2,
  losing regression coverage for normal reuse. NOT integration-ready.
  Same Luna owner retains heavy slot to produce v2 preserving original scenario
  plus separate stale-tail regression, compiling RED and both-tests GREEN.
  V41 evaluator `ses_e88a214ffffeUUkvoFWaknNEvj` independently reviews immutable
  v1 production approach; exact v2 candidate still needs renewed review once
  delivered. No canonical integration or installed acceptance yet.
- OAuth researcher delivered Rust-specific `rust-gap.md` under original external
  browser research directory, reported SHA256
  `bfbeab54fd489c937d18434700492a255d1c0441d1b58b11150a901a441c5bb2`.
  Corrects earlier PKCE/refresh and epoch-expiry confusion; identifies existing
  device/persistence support and missing browser listener/begin/complete flow.
  Same v41 worker now implements ONE coherent private browser-login package,
  granted private provider lib/new browser module, server lib/integration/new
  callback module and NEW tests. No canonical/dependency/deviceauth edits or
  execution permission; compaction still owns heavy slot. Browser method cannot
  be advertised as supported before end-to-end wiring and validation. Security
  review and exact candidate tests will be required; no implementation delivered
  by the research addendum itself.
- Corrected EOF reviewer `ses_e88b92804ffeagmejD1Ya47dQS` now APPROVES exact
  `f3ea4ccfe30ff57df193048cfed7632a4dcb975593625696e9647c67da1e8ff5` after
  retracting wrong-path/wrong-file claims. Source-only: no compilation/execution
  of corrected fixture. Review note external
  `.../T/opencode/eof-correction-review-v41-20261026/scratchpad.md`, reported
  SHA256 `ee6f6040454b908f9db3646fe0933d3b7cd7aa625b4dfa9a15ec6891807a31c7`.
  Original negative evaluator `ses_e88fb28c8ffeG9bPmJ5CMfwxn0` reassigned on
  allowed v41 to review new hash; old3a4d55approval does not transfer. Validation
  remains queued after compaction heavy slot; no integration/acceptance yet.
- Original EOF evaluator renewed APPROVE on exact f3ea4ccf... . Coordinator then
  executed corrected private fixture: **12 passed, 0 failed** in 6.52s,
  `device-edge-compile-proof-ssxngo_a/corrected-tests.stdout`, receipt adjacent.
  Still not integrated; new fixture source reviewed by both evaluators.
- Compaction v2 actual green-v2.log shows BOTHtests PASS; red-stale-v2.log
  shows compiled failure, red-validreuse-v2.log shows originalcasePASS. Saved
  `test-v2.rs` differs actual source by final conditional summary-count assertion;
  coordinator stopped manifest assembly on hash mismatch rather than claiming
  exact-source provenance. More importantly independent reviewer raised substantive
  upstream parity issue: bounded new tail should be retained with existing held
  summary, not force another summary. v1/v2 NOT integration-ready. Luna reassigned
  to v3 preserving within-budget reuse plus appendedtail, existingidentity/tail
  limits/storageheadguard; source-hashed RED/GREEN required. Luna again owns
  heavy slot after auth test run completed. No negative finding outvoted.
- Exhausted Token Harbor also failed last-tab author
  `ses_e88be529dffenPmnrU7qap3J5e` and leader evaluator
  `ses_e88bae68bffes0sGBMTdOX7Dqj`. Allowed v41 replacements launched:
  `ses_e889bc75affe4bRlbbc035kDvk` (private actual last-tab regression),
  `ses_e889cf0c2ffeB1ZYzmZezNb3R3` (preserved leader candidate evaluation).
  No Token Harbor retry. Unexpected web/ source edits observed in canonical
  status; provenance unknown, preserve as user/other work and do not overwrite.
- Tab fixture v41 author returned no files, read wrong truncated horizontalcopy,
  mistyped output path, and falsely reported strategy note nonexistent. Coordinator
  verified correct assigned horizontalbaseline EXISTS with950lines and strategy
  note EXISTS with78lines; created writable exact external output directory
  `.../T/opencode/tabs-fixture-v41-recovery-20261026/`. Same author narrowed to
  ONEverticalfixture correction, write actualartifact before any more broad
  research. Horizontalrepair still incomplete; no path/ACL diagnosis accepted
  from misspelled-directory errors. Model-label-only partial must not be called
  ledger identity/installed acceptance.
- Leader evaluator `ses_e889cf0c2ffeB1ZYzmZezNb3R3` statically approved preserved
  patch (reported hash prefix8aaf4a46), but no compilation/execution. Its phrase
  ready-for-integration is premature. Coordinator found new tests call existing
  select/close transition helpers or pure tab lookups rather than new Ctrl-X
  dispatch, so they may pass pre-fix and fail to detect missing wiring. Same
  evaluator reassigned to assess this gap and propose actual dispatch regression.
  Static production approval retained, test adequacy unresolved; integration held
  for focused execution and meaningful regression. Heavy slot still compaction.
- Browser implementation delivered two private draft modules but no wiring or
  executed tests. Coordinator inspected concrete blockers: callback spawn drops
  listener/cancelreceiver; HTTPread_line unbounded/no deadline; provider accesses
  private AuthTransport.client; direct ring dependency absent. Mark UNREADY and
  assigned sameworker to repair owned bounded runtime/interfaces first. Allowed
  PRIVATE providerCargo.toml ring dependency proposal only, no canonical changes.
  No registration until listener exists before authorizeURL, cancellation works,
  exchange/persistence flow wired and independently verified.
- Last-tab v41 author again delivered noartifact. Narrowed to ONEactual function
  plugged intoexistingJourneyharness, targetedreadsonly/writeearly. Corrected its
  proposed assertion: new session receivingnewprompt must contain thatprompt,
  notremainempty. Oldsessionhistorymustremainunchanged. No newbrowser/provider
  harness rewrite; no execution authorized during compaction slot.
- Compaction v3 author returned NEEDS_FIX, nofinalpatch/hashmanifest. Production
  draft reconciles live suffix into held checkpoint to preserve upstream reuse.
  Coordinator inspected actual test lines427-432: it still requires original
  summary request to CONTAIN tail inserted AFTER requestcapture, a contradictory
  v2resummaryexpectation. Correct v3 temporalcontract: summaryrequest excludes
  newtail; checkpointretained andforegroundcontinuation include newtail; exactly
  one summary reused. Author reassigned to freezecorrectedtest-onlyRED, retain
  strongcontentassertions, runbaseRED andcandidateboth-testsGREEN withhashes.
  Typedtoolpairtail remains unproven; don't claim User regression coversit.
  Heavy slot remains exclusively Luna; no integration authorized yet.
- Compaction v3 final artifacts now exist. Coordinator verified candidate patch
  SHA256 `0dd7ca4f87bb9cb7ef3d0ea58b3bf756886468a60fe952d1aa9be18af419a7cb`,
  test-only patch `cbb3062f37f623af0dd72e710ce8d2b1ab2f80ac37076cd708953d2a2956852f`,
  manifest `4130707bd0c4747455bfdee2cb78cc05aaecea83f0bb66c549d95ec3240eab9b`.
  Actualsourcehashes match worker finalproduction69f9fa23... andteste9095767... .
  Worker reports baseRED andcandidate2PASS withonesummaryreuse, tooltailuntested.
  Original evaluator `ses_e88a214ffffeUUkvoFWaknNEvj` and second independent
  `ses_e88874839ffe8MSRh1o60l6TBG` reviewing EXACTv3hash. Content-only overlap
  dedup deserves scrutiny for repeatedmessages; no integration before verdicts.
- Luna released heavy slot. Coordinator now runs scoped macOS PID RED/GREEN
  in existing private `context-exact-integrated-jjhruynv/` tree, exact reviewed
  RED/GREEN patch hashes checked. Shell `sh_177790226001Bk7t3T7nZ2vxGk`, logs
  `pid-{red,green}.{stdout,stderr}`, sourcehashes/commands/exitcodes in PID artifact
  `coordinator-test-receipt.json`. No canonicalcode/daemon/userDB touched.
- PID initial run compiled: base9PASS/2FAIL; candidate10PASS/1FAIL. Real
  current-process liveness regression corrected by production fix, but descriptor
  test wrote data/backend.json instead of actual data/runtime/backend.json,
  causing Absent in BOTHphases. Coordinator repaired private fixture using
  `DaemonPaths::for_data_dir(...).descriptor` and parentdirectory creation;
  candidate rerun shell `sh_17779c4d5001f9EJPDYkRRSQJk`, logs
  `pid-corrected-green.*`. No PASSclaim until completion. Corrected fixture
  invalidates previous testhash and requires renewedreview plus cleanRED.
- Verticalfixture author again returned NOartifact and repeated misspelled
  `/var/...dj81nc_j2qyp2...` path/ACLclaims. Assignment stopped; no further
  research pass. Correct outputdirectory remains coordinatorverified writable at
  exact `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/...`.
  Verticalfixture implementation still incomplete; authorreportnotacceptance.
- Corrected PID regression now has meaningful actual RED/GREEN: baseproduction
  withcorrectdescriptorfixture compiled9PASS/2FAIL (livePID+discoverReusable),
  fixedcandidatefullCLIsuite **851PASS/0FAIL**. Logs inprivateCLI tree
  `pid-corrected-red.stdout`, `pid-final-full-cli.stdout`. Candidate+hashes+commands
  atPIDroot `coordinator-final.{patch,receipt.json}` and
  `coordinator-final-candidate.rs`. Correctedfixturegreen11PASS includes app002
  selector'sexistingtests aswellas4newtests; don'tclaim11newtests.
  Two exactcandidateindependentevaluators assigned beforeintegration:
  `ses_e88b92804ffeagmejD1Ya47dQS` (correctness/security) and
  `ses_e88852fd1ffexRUvwa4HvtugzG` (secondreview). Candidateonlynotintegrated;
  installedsecondclientEOFresolutionstillunproven. Heavy slotreleased.
- Leader evaluator confirmed all five new tests are NOT_REGRESSION: they bypass
  byte dispatch and pass baseproduction. Explicit NEEDS_FIX coverage verdict,
  static production approval only. Note SHA256
  `29214f29c9d12098200e2be86d2601e34bd4f57c25f173d79b18c5babaf12c2b` at
  `.../T/opencode/leader-v41-review-recovery-20261026/scratchpad.md`.
  New private author `ses_e888ddac0ffeu32szuvXRKUZQO` assigned actual live-used
  dispatch seam plus byte/installed regression, external artifacts only. No
  source-text assertions or duplicated test-only matching accepted. Candidate
  changes require new independent review; no integration/acceptance yet.

- Leader author `ses_e88d5600fffepyWKGmG4cZKga7` wrote canonical
  `crates/cli/src/tui_entry.rs` contrary to its private-copy grant. Actual diff
  matched its reported 171 insertions/6 deletions. Coordinator preserved exact
  file, base, binary diff, SHA256 hashes and receipt externally at
  `.../T/opencode/leader-canonical-delta-preserved-56sk_j24/`, then hash-checked
  and reversed only that completed worker's exact delta. Canonical product file
  restored to committed base; no unrelated files changed. Token Harbor evaluator
  `ses_e88bae68bffes0sGBMTdOX7Dqj` reviewing preserved candidate. No build/test
  results or integration approval exist yet.
- PTY researcher returned lifecycle evidence at
  `.../T/opencode/pty-research-20261026-9router/scratchpad.md`, reported SHA256
  `ee439e362b0c55d7b4779751680ba3ac8229a5849094cc8bb4aeb6636a00b495`.
  Distinguish pipe-backed shell streaming, in-process PTYs and persistent daemon
  PTYs; unused gateway state-machine code is not a wired runtime feature.
  The worker's assertion that persistent PTY is outside scope is NOT authorized:
  persistent daemon-owned work remains a user requirement. Revalidate precise
  source claims before implementation; no end-to-end continuity acceptance.
- Composer ACK audit `ses_e88d69cd4ffeXQ7i0yUzX5nXew` returned NO_PASS but
  its argument is internally inconsistent: differing drafts are preserved by
  the helper, and it did not account for `binding_active` around event draining
  (`tui_entry.rs:3645-3655`). Need a concrete same-text cross-tab/generation
  regression before accepting its proposed per-tab pending-submission redesign.
- GLM last-tab regression handoff confirmed no artifact was ever written; a tool
  command timed out during follow-up. Pause new GLM dispatches under stop-on-error
  rule; token balance UNKNOWN. Replacement Token Harbor author
  `ses_e88be529dffenPmnrU7qap3J5e` owns actual private Flow C artifact.
- Luna `ses_e88bd254fffeg0P1UZYgSytZK9` owns the exclusive heavy slot for a
  real compaction tail-retention RED/GREEN in private copies of
  `auto_compaction.rs` and `background_compaction_runtime.rs`. Preserve storage
  head-identity and retained-tail bounds; do not schedule concurrent Cargo/PTY.
- Combined-route completions continue arriving from previous launches, but new
  assignments remain paused after no-eligible-account failures. Current useful
  concurrency is not verified at 20; route availability and incomplete handoffs
  constrain replenishment. Record launches/completions rather than claiming all
  twenty original assignments remain active.

## Latest verified delivery status — PID and TUI

- MacOS PID final candidate: private artifact directory
  `.../T/opencode/macos-pid-v41-recovery-20261026/`,
  `coordinator-final.patch` SHA256
  `d76416947b6fae48e52e17d663505882bc986323c7bae36e405f86734dbff3ee`,
  candidate source SHA256
  `2f4b378f06aec9cb6c8f2c8649c71c89ef1dbee4a6fc437d9dbeaf507dc6d535`.
  Corrected descriptor fixture uses production DaemonPaths runtime location.
  Base production compiled with two actual failures; fixed candidate passed
  **851 CLI tests**, raw logs `context-exact-integrated-jjhruynv/pid-corrected-red.stdout`
  and `pid-final-full-cli.stdout`. Second evaluator
  `ses_e88852fd1ffexRUvwa4HvtugzG` APPROVED exact hash. First correctness/security
  evaluator `ses_e88b92804ffeagmejD1Ya47dQS` still pending. No integration or
  installed second-client acceptance yet.
- User explicitly challenged lack of installed visible progress and complete
  OpenCode TUI parity. Acknowledged complete pinned V2 TUI is NOT delivered and
  real installed oc2 has NOT been updated. Existing upstream source, scratchpads
  and six rendered reference captures suffice for concrete Rust comparison;
  avoid another broad research cycle. CLI test counts do not prove visual parity.
- Compaction v3 exact patch SHA256
  `0dd7ca4f87bb9cb7ef3d0ea58b3bf756886468a60fe952d1aa9be18af419a7cb`
  under Luna private source/evidence/ is undergoing two independent reviews
  (`ses_e88a214ffffeUUkvoFWaknNEvj`, `ses_e88874839ffe8MSRh1o60l6TBG`).
  Reported 2 focused PASS preserving one-summary reuse. Content-only overlap
  dedup requires scrutiny for legitimate repeated message loss. Not integrated.
- Leader implementation worker returned byte-identical old source706a63b0...,
  unchanged seam tests plus documentation, NOT requested coverage repair.
  Rework loop stopped. Coordinator wrote actual private Python Journey function
  `.../T/opencode/leader-installed-coordinator-20261026/native_leader_journey.py`,
  SHA256 `e025bf5774e959a1c6e349cb2d43da42c9a7fc87ae036c85731a005c1acc57fa`.
  Feeds real Ctrl-X+1/2/w bytes, asserts authenticated session selection through
  existing screen identity predicate, last-tabHome, durable session preservation,
  no provider calls and daemon alive. ASTsyntaxPASS only; needs runner integration,
  independent fixture review and actual base/candidate installed execution. No
  product acceptance or canonical product edits from this test authoring.
- Browser author returned another incomplete draft: private provider transport
  and redactedPKCE changes reported, but callback accept-loop/completion wiring,
  regressiontests and finalmanifest remain missing. No compilation/execution.
  FREEZE browser draft under `.../T/opencode/openai-browser-implementation-v41-20261026/`;
  preserve files, do not integrate partial registration or advertise support.
  No further OAuth rework assignment launched. Prioritize user's repeatedly
  requested installed TUI parity and delivery over expanding this auth draft.
- Original compaction evaluator returned supportive exactv3analysis but no
  explicit approval verdict; do not translate supportive prose into acceptance.
  Same evaluator asked for concise APPROVE/NEEDS_FIX with concrete orderedhistory
  proof for overlap/repeatedmessage multiplicity and fullrequestbounds. Second
  evaluator still pending; candidate remains private despite focused2PASS.
- PID security evaluator found valid v1missingnonUnixcfg concern: explicit
  NEEDS_FIX blocks old2f4b...candidate despite secondapproval. Coordinator added
  exactserver#[cfg(unix)] nativeprobe and#[cfg(not(unix))] conservativepositivePID
  fallback inPRIVATEtree, retainingzero/outofi32guards. New immutable
  `coordinator-v2.{patch,rs}` (candidate file namedcoordinator-v2-candidate.rs)
  andreceipt inPIDartifactroot. ActualfullCLIrerunexit0 at
  `context-exact-integrated-jjhruynv/pid-v2-full-cli.stdout`; noWindowsbuildclaim.
  Bothoriginalevaluators reassigned toexactv2; oldapprovalnottransferred.
- Compaction second reviewer `ses_e88874839ffe8MSRh1o60l6TBG` explicitly
  APPROVED exactv3. Original evaluator still returned truncated reasoning with
  no verdict; requested bounded <=180worddecision without more broadresearch.
  JSON equality by itself is not message-identity proof; retained/suffix ordering
  invariant must justify multiplicity preservation. No integration yet.
- PID original evaluator returnedAPPROVE but cited stale v1hashes. Actual v2
  receipt candidate `fe3076ff93da5db8ae37e39cb1c8de241ac07a85a9928bde4bd5ce5081017362`,
  patch `103245c4cd9aaf5d3a62d925b6699d1d6b6669579ed7d316931c8b779a4c9273`.
  Reassigned onlyhashidentitycorrection+explicitverdict, max3tools, no repeated
  research. Do not silently reinterpret approvalof2f4b...asapprovaloffe3076... .
- PID second evaluator explicitly APPROVED correct v2fe3076.../103245... hashes.
  Coordinatorverified candidate, patch and actualreviewnoteSHA256
  `51e2fba8f168d63b1c513e8a0d5a36838e2f08308b94a368e39978c5e6740bf9` at
  `.../T/opencode/pid-final-secondreview-20261026/v2-review.md`.
  FullCLI851PASS confirmed. Onlyoriginalsecurityevaluator'salreadyrequested
  exacthashcorrection remains before scopedintegration; do not launch another
  broadreview orholdcandidateforunrelatedfeatures afterthatclears.
- PID firstsecurityevaluator explicitly corrected approval to v2fe3076... and
  patch103245...; both independent approvals now cover SAMEcandidate. Coordinator
  applied exactreviewedbytes via scopedpatch, hashverified, committed ONLY
  `crates/cli/src/daemon_client.rs` as
  **`c821e522968217a5242e6c9da1eccc0068f2255c`** on main-v2. Integrationreceipt
  externalPIDroot `integration-v2-receipt.json`. CandidateCLI851PASS; exact
  integratedrun/installedsecondclientjourney still pending. RealuserbinaryNOT
  updated bythiscommit. Otherworkingfiles preserved; command-localhookoverride
  under Phase1instructionusedonlyforthisscopedcommit.
- Compaction originalevaluator explicitly NEEDS_FIX v3content-overlapidentity;
  secondapprovalcannotoutvote. Luna assigned v4identity/captured-endrepair plus
  repeatedidenticalrowregression, owns serializedheavy slot again. No compaction
  integration authorized; preservev3negativeuntilnewexactcandidate reviewed.

## Continuous delegation replenished — 20 new launches

User again required continuous20+usefulassignments. Launched20independent
backgroundassignments below on allowed routes: sixvyce-deepseek-v41 (within8cap)
and fourteen9router-luna-free. Oldercompletedworkers are NOT counted as active.
Launchreceipts are not proof allwillremainrunning; replenish oncompletion/error
withpendingindependentwork ratherthanwaitingforbatch. All use committedbase
`c821e522968217a5242e6c9da1eccc0068f2255c`, selectedupstream450e7b9, role-first
prompts, privateexternalartifactgrants andscratchpads. NOworkerbuild/PTY authority;
compaction'sexistingheavy slot remains serialized. Token telemetry UNKNOWN;
GLM/TokenHarbor/combinedroutes remain paused per recorded failures.

| Session | Role/deliverable | Route |
| --- | --- | --- |
| `ses_e8862883fffebgXKQs6mA57Uap` | Decoder tab-key patch/tests | v41 |
| `ses_e88628838ffeHyCY0ZRyQ648UB` | Vertical installed fixture | v41 |
| `ses_e88628827ffeM293oX7j021F0x` | Horizontal installed fixture | v41 |
| `ses_e88628826ffeTUoylhew03fx8S` | Context click/slash journey | v41 |
| `ses_e8862880effer444k32wVdS8hd` | Last-tab Home fresh-session fix | v41 |
| `ses_e8862880effdk2n0mBfIUi9H2k` | Rust actual-render capture fixture | v41 |
| `ses_e8862880dffeZL1mol1A759VAM` | Composer metadata parity patch | luna |
| `ses_e88628805ffeuX2ybBvbp6XOy3` | Command palette parity patch | luna |
| `ses_e886287fbffe9wj9n3E5mqJ6W2` | Model picker parity patch | luna |
| `ses_e886287edffenGd9b3jBds8IC9` | Approval dialog parity patch | luna |
| `ses_e886287edffdTdyPnsKwXbJent` | Session picker parity patch | luna |
| `ses_e88628788ffeHlnA7H0lbgsfrO` | Tool transcript projection patch | luna |
| `ses_e88628777ffeMqGnEa0QC3PCS4` | Markdown render parity patch | luna |
| `ses_e88628775ffejvpYL51wPUGj0r` | Text selection parity patch | luna |
| `ses_e8862872cffeIEE4c80F3IHHxn` | Actual reference theme differential | luna |
| `ses_e8862872bffenCeesh7rgZPA24` | Exact-release executable runner | luna |
| `ses_e8862872bffdQ62javDqxVyLnk` | Bounded TUI acceptance matrix | luna |
| `ses_e8862872afferJ5qk8Sazrza40` | Keybind override parser candidate | luna |
| `ses_e88628729ffeN9jZVXc7Ku5Ac2` | Real-byte leader journey review | luna |
| `ses_e88628728ffew6zpoCk0VjIfFY` | Last-tab installed runner completion | luna |

Privateoutputs: exact absolute approved macOS temp root plus
`replenish20-01-decoder/` through `replenish20-20-lastrunner/` as named inprompts.
Workers must deliveractualpatches/scripts/hashes, notanotherbroadresearchsummary.
Unexpecteduncommittedweb/serverauth/lib/web_distchanges are preserved; workers
may not overwrite them or claim them as reviewedintegratedcode.

### Prompt replenishment events

- Modelpicker worker `ses_e886287fbffe9wj9n3E5mqJ6W2` completedwithoutpatch,
  testsorscratchpad; inspectedwebappnotTUIreference. INCOMPLETE. Immediately
  replaced by `ses_e885e1a87ffeeidQ5htZxnRxod` onallowedLunaroute, narrowedto
  actualTUI dialog-model/native_setup search/order withwrite-early requirement.
- Acceptanceauditor `ses_e8862872bffdQ62javDqxVyLnk` completedwithoutmatrixor
  scratchpad. INCOMPLETE. Immediately replaced by `ses_e885e1a85ffeVYuz7QLnZGI9P3`
  onallowedLuna, instructedtowritecollected12-rowdraftbeforefurtherresearch.
- These two completions replacedone-for-one promptly, preserving20launched
  usefulassignments absentothernotifications. Noartifactcompletionclaimed and
  no heavyvalidation permissions added. Route token telemetry remainsUNKNOWN.

### Further immediate replenishment

Acceptance matrix delivered at `replenish20-17-acceptance/matrix.md`, read by
coordinator; documents UNPROVEN journeys, not installed acceptance. Replaced by
`ses_e885b1ea2ffeX8UrwRlqM2YR15` daemon reconnect regression (`replenish20-21-reconnect`).
Six further completions replaced promptly, all allowed `9router-luna-free`:

- Compaction author resumed `ses_e88bd254fffeg0P1UZYgSytZK9` to finish v4
  captured-end identity repair. Current edits UNVERIFIED; owns only heavy slot.
- Palette author delivered unexecuted private source; questionable settings to
  appearance alias goes to `ses_e885ab270ffeHrDkZoRnsivcGB` independent review
  and search-only correction (`replenish20-22-palettereview`).
- Leader review NEEDS_FIX e025...: selected ID incorrectly included in other_ids.
  Repair assigned `ses_e885a7e08ffel0ERWN71KFJOVQ` (`replenish20-23-leaderfix`).
  No acceptance of impossible predicate or bypass of actual input-loop proof.
- Keybind parser module delivered 13db5782... UNREADY without runtime wiring.
  Independent schema review `ses_e885a55c3ffe1rjs8Yy9adBWB5`
  (`replenish20-24-keybindreview`).
- Six-frame theme comparison delivered ea2447df...; source-only user rail color
  mismatch unresolved. Targeted paint precedence task `ses_e885a2ec7ffe7lODxGqg9sF317`
  (`replenish20-25-railcolor`), no rendered parity claim.
- Native model subsequence patch delivered 1e6f61cd...; ranking remains partial.
  Independent review `ses_e8859f385ffex5tr5nm3wdMzNj`
  (`replenish20-26-modelreview`).

All new artifact directories are under the same exact approved external root
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`.
No new build permission except existing compaction slot; canonical unchanged by
workers. Twenty-assignment launched roster replenished one-for-one from notified
completions; actual execution state only as harness reports, usage UNKNOWN.
- Selection worker completed with incomplete private copy and no hashes/tests;
  immediately resumed `ses_e88628775ffejvpYL51wPUGj0r` for bounded click-versus-drag
  patch/tests and mandatory scratchpad/hash delivery in its existing grant.
  No canonical changes or clipboard actions authorized; no progress claim for
  incomplete edit. Replacement preserves one-for-one useful work replenishment.
- Tool projection patch delivered `c344767dee824b5e105795e103e001efa7c39fd9ba06d848f99f3244f9abb420`,
  unexecuted; worker replaced immediately by independent review
  `ses_e8858fea9ffeItZ1GIXnRtNN88` (`replenish20-27-toolreview`). Full collapse/
  expansion still requires renderer integration; no claim delivered.
- Markdown private source delivered `31d87d1cc5d130dc77bdc0d11018bbcb56225cbc54f050a00d1b2077511ac4ac`,
  unexecuted; replaced immediately by `ses_e8858daf1ffeMcw3IglEkcob6z`
  (`replenish20-28-markdownreview`) checking actual live path as reported whitespace
  defect is in native_transcript.rs but candidate changes native_view_chat.rs.
  Both reviewers read-only canonical, unique external notes, no build authority.
- Last-tab runner delivered cb3d8544... /1546cab4..., immediately replaced by
  `ses_e8858252bffetdhVRmXDIJTTxm` independent fixture review (`replenish20-29-lastreview`).
- Rail investigator source-level mismatch lacks render proof; replaced by
  `ses_e8857f48affezRikwlDtb8zKGA` read-only scoped delta proposal
  (`replenish20-30-raildelta`), no shared native_view write grant.
- Metadata patch 18400f50... delivered; replaced by independent evaluator
  `ses_e8857d060ffe9A8vfXrLsJ3ka7` (`replenish20-31-metadatareview`).
- Coordinator independently verified six reported file hashes (metadata/model/
  tool/markdown/two last-tab Python files) and AST parsed both Python files.
  Receipt `replenish20-coordinator-receipts/delivered-hashes.json`. This is artifact
  identity/syntax validation only, NOT compilation or installed acceptance.
- Session-picker worker unfinished tests/notes: immediately resumed same session
  `ses_e886287edffdTdyPnsKwXbJent` with <=9-tool bounded finishing assignment.
- Daemon reconnect draft c4a03c77... delivered; immediately replaced by independent
  ownership/identity evaluator `ses_e88573e98ffehVRZrhXqLXvrKZ`
  (`replenish20-32-reconnectreview`). Must check stop helper doesn't kill daemon
  before reconnect and descriptor proves same fixture daemon. UNEXECUTED.
- Subsequent immediate replacements, all allowed Luna with private external grants:
  selection delivered 597aa4b9... patch, evaluator `ses_e8856afdfffemWDOlBlNqWUKYZ`
  (`replenish20-33-selectionreview`). Model limited subsequence APPROVE received
  (not ranking parity); integration prep `ses_e88567d20ffeONwXBeqYt1BYV8`
  (`replenish20-34-modelready`) awaits serialized validation, no new feature scope.
- Keybind review NEEDS_FIX integral-decimal timeout; optimizer
  `ses_e885601dbffed3XlyNjjkdvx2Z` (`replenish20-35-keybindfix`).
- Markdown review NEEDS_FIX silently capped Unicode clusters / nonASCII fence
  closer whitespace; optimizer `ses_e8855ce94ffeD3JygYoeiInSM4`
  (`replenish20-36-markdownfix`). Original negatives remain blocking.
- Corrected leader fixture 8b8edb4a... plus runner24ef9035... goes back to ORIGINAL
  evaluator `ses_e88628729ffeN9jZVXc7Ku5Ac2` for exact v2 rereview.
- Release runner unverified; evaluator/finisher `ses_e88551404ffeY9Wy3dwnWf2qFt`
  (`replenish20-37-releasereview`) must use clean exact-commit archive, preserve
  dirty canonical, verify helper flags/rpath, no release execution permission.
- False settings=>appearance rejected, search-only correction not accepted yet.
  Native settings state candidate `ses_e8854e5f4ffebTEg9A48uv1Yqo`
  (`replenish20-38-settings`) owns NEW module only, shared runtime wiring ungranted.
- Sessionpicker candidate delivered, evaluator `ses_e8854ba16ffeJfAWod5mqg7vcN`
  (`replenish20-39-sessionreview`) computes missing exact hashes and checks callers.
- Metadata review NEEDS_FIX 9-cell minimum/provider-boundary/Unicode/hitbox tests;
  optimizer `ses_e88548dc1ffeVba5NM66ICKhgu` (`replenish20-40-metadatafix`).
  Original 18400f50... not eligible; corrected exact candidate needs renewed review.
All above replaced notified completions promptly; no old completed worker counted
as running. Builds still only in existing compaction heavy slot. No private
source-only approval represented as integrated or installed product PASS.
- Model preparation found malformed hunkcounts in approved1e6f61cd... patch;
  same `ses_e88567d20ffeONwXBeqYt1BYV8` resumed mechanical-only count repair,
  preserveRustlogic, compute exact candidate hash. No valid applyproof/PASS yet.
- Reconnect NEEDS_FIX same-descriptor/session assertion and swallowed cleanup;
  optimizer `ses_e88538e04ffeTo8LSUzmaVNMcZ` (`replenish20-41-reconnectfix`).
- Selection source APPROVE received; exact candidate preparation
  `ses_e885318e7ffe05iOkeXN2T9wFo` (`replenish20-42-selectionready`) no test execution.
- Corrected leader fixture/runnner original evaluator APPROVE exact8b8edb4a.../
  24ef9035...; production minimal delta prep `ses_e8852daadffethin7HTqZjkwg4`
  (`replenish20-43-leaderready`), shared source readonly proposal only.
- Render worker delivered only scratchpad; immediately resumed
  `ses_e8862880effdk2n0mBfIUi9H2k` bounded actualNativeView grid fixture,
  no more bridge-only investigation. No six-Rust-screen capture delivered yet.
- Decoder worker no patch; resumed `ses_e8862883fffebgXKQs6mA57Uap` bounded
  private decoder/test completion. Functional keycodes must be verified, not
  conflate AltUp/Down with ASCII Backspace/Tab. No product decoding claim yet.
- Horizontal fixture worker no file; resumed `ses_e88628827ffeM293oX7j021F0x`
  write-first finishing. Corrected its plan: ALL persisted sessions != OPEN tabs;
  track open UUID set separately, preserve closed-session ledger. Duplicate Chat
  labels cannot alone prove selected identity. No title-fixture acceptance yet.
- Context worker misread mouse click as invented /contextclick command and found
  it absent upstream; corrected/resumed `ses_e88628826ffeTUoylhew03fx8S`: actual
  mouse hitrect plus /context LOCAL extension, no upstream-availability blocker.
All completions above replenished promptly. No additional product commit from
these handoffs; private deliverables need exact review and serialized verification.

## User context screenshot requirement and continuing execution

User explicitly supplied Claude Code `/context` text + screenshot and repeated
"this is how I want my /context to be". Durable specification:
`docs/CONTEXT_USAGE_UI_REQUIREMENTS.md`. This is the approved extension reference:
colored grid, model/capacity headline, estimated categories, MCP tools/memory/
skills trees and evidence-based suggestions. Actual request accounting required;
inventory page estimates cannot masquerade as entire request. Unknowns remain
unreported. Mouse context indicator and /context same display, no /contextclick.

Three useful parallel assignments replace latest completions:
- `ses_e884a754affe67GhqY705ThGFn`: private native_context renderer,
  `context-user-01-render/`.
- `ses_e884a7545ffenjzK3cSw9agMtH`: request-accounting/payload reuse and patch,
  `context-user-02-accounting/`.
- `ses_e884a7544ffeCnDf7sRAKY1LVL`: screenshot-derived acceptance fixtures,
  `context-user-03-acceptance/`.
All allowed Luna, private external outputs, no Cargo/PTY execution.

Selection candidate independently compiled/tested **13 PASS** against clean
archive c821e522; integrated exact reviewed bytes as
`5cdbb9892a9087283c64da33d67c4a86718bf8ac`. Only native_text_selection.rs committed.
No actual installed oc2 update. Exact-integrated selection + model/session
candidate focused verification now runs in serialized background shell
`sh_177b5fdae001QOMbncECpP8xw3`; no other worker has heavy validation grant.
First archive attempt failed due old Python extractall(filter=...) BEFORE tests;
corrected archive path validation and rerun passed 13 tests, no fabricated pass.

Compaction v4 valid patch generated by coordinator with difflib, reverse apply
check passed; patch `58dcc236d6923f37043498dea2593ef81e9b2e75d055fd81702929a6dd55c67a`
under existing source/evidence/coordinator-v4/. Original evaluator APPROVE,
second evaluator `ses_e88508938ffe0kMDgBYGW5pZTf` NEEDS_FIX separate-read race.
V4 BLOCKED. Optimizer `ses_e88bd254fffeg0P1UZYgSytZK9` resumed v5 source-only;
must bind captured end to exact precompute snapshot, not separate history read.

Further prompt replacements (all allowed Luna unless noted):
- Timeout schema conflict independently resolved via actual Effect isInt using
  Number.isSafeInteger. Original evaluator corrected its incorrect v2 negative
  and APPROVED exact77890eb9... in v3 note4041d5b1...; runtime still UNREADY.
  Replenished by `ses_e884bd6baffebJWZ8BgsiPMxaR` keybind wiring proposal
  (`replenish20-59-keybindwiring`).
- `ses_e884ff79dffeh3gT15524CGoq5` rail review NEEDS_FIX double tab offset;
  optimizer `ses_e88499c41ffe1pMOLZf8NGhobd` (`replenish20-61-railfix`).
- `ses_e884ff717ffe6TFZw2ztOBMbT9` delivered approved session candidate applyproof;
  replaced by `ses_e884d6b32ffeUGFhm6BTENdWbI` subagent regression
  (`replenish20-55-subagentjourney`).
- `ses_e884ff70cffe7DcBInKXDXXIyn` second-client key patch source approval is now
  DISPUTED: later exactbase audit found producer finally overwrites success JSON,
  leaving requests LIST and provider_requests INTEGER. Ubuntu auditor
  `ses_e884c1d42ffepDh19UIOnCE4O3` correctly delivered NO_CHANGE evidence
  be5cdb92.... Original keypatch evaluator `ses_e884e86e6ffeWECfkV0VCszXYh`
  immediately reassigned final-write schema review; no consumer keypatch landed.
- `ses_e884e86d6ffeGgeYT6l2prwmnJ` leader dispatch NEEDS_FIX stale control-byte
  chord state; optimizer `ses_e884c1d16ffe9iSpZSH45Jw4Rf` (`replenish20-58-leadercancel`).
- Reconnect v2 ebdaa9a7... cleanup corrected but original evaluator NEEDS_FIX:
  reading descriptor doesn't prove second UI actually attached. Not accepted.
- Last-tab installed runner static APPROVE cb3d8544.../1546cab4...; execution
  preparer `ses_e884d6b47ffeGTG8YHz96F1rKS` returned wrong-path/access claims
  and no artifact. No acceptance. Replaced capacity with requested context work.
- Permission UI private patch b4a38831... delivered; security evaluator
  `ses_e88499c77ffeO60qQusdfYE6y4` (`replenish20-60-approvalreview`).
- Settings model e5d94586... NEEDS_FIX Text/OwnedText equality theme cycle;
  no /settings feature advertisement; model and runtime remain unready.
- Vertical v41 author resumed same session write-full-baseline finishing.
- `ses_e884d6b42ffe0iw7eAv8GSoDfl` shell preview/persistence fixture
  (`replenish20-54-shelljourney`) and `ses_e884d3d3bffelhHV5wUVadcIXU`
  composer preservation fixture (`replenish20-56-composerjourney`) active at launch.
Continue prompt replenishment on notifications. Token usage UNKNOWN; paused
routes remain paused. Preserve dirty user web/server edits and daemon73389/4096.
- Serialized shell completed: exact-integrated selection filter PASS on5cdbb989;
  approved model and session candidate focused filters both exit0 in clean snapshot
  `selection-integrated-model-session-381lsgqg/`, receipt.json. Model/session not
  yet integrated; no installed acceptance. Heavy slot now released.
- Shell fixture evidence shows !command executes, not preview; stream drop cancels
  owner and remotePTY is model-only. Repair prep `ses_e884887edffeyyk7bYLyLcSb2N`
  (`replenish20-62-shellownership`) replaces completed worker, no execution grant.
- Second-client consumer keypatch original evaluator RETRACTED approval, now
  NEEDS_FIX5ea88225...: finally writes requestsLIST/provider_requestsINTEGER.
  Keep existing requests/exactfour assertion. No erroneous keypatch integrated.
  Replaced with auth test integration prep `ses_e88484cb6ffe0wJhodFU8w7Syu`
  (`replenish20-63-authedgeready`), no auth behavior or canonical changes granted.
- Markdown corrected diff2ca68a2b... returned, original evaluator resumed
  `ses_e8858daf1ffeMcw3IglEkcob6z` exact v2 source review, originalnegativeblocking.

## Ready feature integration and notification replenishment

- Integrated reviewed/tested model subsequence candidate8663374d... as
  `bace0c71` (native_setup.rs only); reviewed/tested session wrap/refresh candidate
  7bbe8d91... as `b797acda` (native_view_sessions.rs only). Exact final integrated
  b797acda snapshot filters native_setup and native_view_sessions both exit0;
  receipts/logs `integrated-pickers-yvdj1xl8/`. No installed UI acceptance claim.
- Selection exact-integrated13PASS already recorded. Source integration does not
  update actual user binary. No unrelated web/server edits staged or committed.
- Completed subagent fixture -> reviewer `ses_e88476509ffevG4jiAnm7Bm2zY`
  (`replenish20-64-subagentreview`), NEEDS_FIX stale-frame draft assertion ->
  optimizer `ses_e8843de67ffecvzfrW61KC8mhp` (`replenish20-68-subagentdraftfix`).
- Permission reviewer NEEDS_FIX unused new model/live DialogView remains
  unchanged -> optimizer `ses_e88468ad4ffeLAT1ojAaYyF5nU`
  (`replenish20-65-livepermission`), explicit live save-pattern gate repair.
- Render v41 third handoff still no file; replaced with Luna
  `ses_e88468abcffeIfS4Guw75vxF6a` who delivered actual test3b2c4345... ->
  evaluator `ses_e88444b3affeqqCr514RFXpWNh` (`replenish20-67-renderreview`).
- Context installed v41 third handoff no file; replaced with Luna
  `ses_e88460c02ffepmTWAI6GbGgDG7` (`context-user-05-installed`), new screenshot
  ContextUsage heading/slash/actualmouse route fixture, no fallback TUI research.
- Composer fixture own known empty-string predicate NEEDS_FIX; sameauthor
  `ses_e884d3d3bffelhHV5wUVadcIXU` resumed bounded correction/hash.
- Tool pairing corrected18dd20ff... goes back to original evaluator
  `ses_e8858fea9ffeItZ1GIXnRtNN88` exact v2 review.
- Markdown evaluator found corrected delta808855fe... sound but againstoldproposal,
  notcanonicalbase -> composition prep `ses_e88438f42ffet1HVwQpAn5aH6a`
  (`replenish20-69-markdownbase`), unchangedlogic/exactfinalhashrequired.
- Context golden fixture initial claimed artifact missing (coordinatorglob showed
  onlyscratchpad). Author resumed/materialized actualJSON at exactabsolutepath,
  newSHA170d28c44e33b9a24894782e9c99103f53b64e9c2ca812ed47e3b1508bd5a7bf.
  Priorbdb...hash withdrawn. Original reviewer `ses_e8846df84ffe9tLSREPE846AL0`
  resumed exactnewfile review. No fixturedefinition or parse claimedrenderPASS.
- Auth fixture newfileproposal433cc550... now prepared after correctingfalse
  missing-basepathblocker. Candidatef3ea4ccf exactbytes verified byworker;
  integration stillpending. Replaced worker by device installed regression
  `ses_e884327b0ffe9mc7EtLEoPV5u1` (`replenish20-70-devicejourney`), no real login.
All above replacements were immediate on notifications, allowed routes only;
continued20+launchedusefulroster, completedworkersnevercountedasactive. Main
heavy slot free after exact-integrated picker checks; no subagent build grants.
- Continued notification replacements, allowed Luna unless specified:
  Home v41 draft delivered but not applied -> `ses_e88424fc4ffetAoXo1CSnA2wBH`
  (`replenish20-71-homecandidate`) exclusive new private source candidate grant.
  Context slash fixture partial015cc862... -> sameauthor resumed actualnative
  hitbox and runnable slash/draft checks, no fallback path or fake pointer PASS.
- Rail corrected6904b162... -> original `ses_e884ff79dffeh3gT15524CGoq5` v2 review;
  leader corrected2ae4918c... originalAPPROVE -> preparation
  `ses_e8841d6beffe5fvxqxrxRYOamD` (`replenish20-72-leadervalidate`).
- Composer corrected5eb9b68c... -> evaluator `ses_e8841d6b6ffebB3VQ6sK4kOO5a`
  (`replenish20-73-composerreview`). Legacy heading may be acceptable only for
  preservation regression, never for new screenshot UI acceptance.
- Markdown canonicalbased5897821a... unchangedfinal808855fe... -> original
  evaluator `ses_e8858daf1ffeMcw3IglEkcob6z` v3 identity review.
- Context accounting worker confirms inventory endpoint cannot account assembled
  request. Resumed `ses_e884a7545ffenjzK3cSw9agMtH`, expandedPRIVATE grant for
  base-extractedlib/state/contextaccounting/http copies and actual capture wiring
  diff. Dirty canonical lib/auth userwork preserved; actual snapshot capture
  after requestassembly, bounded identity/provenance, no fake category counts.
- Context golden actual170d28c4... original reviewer NEEDS_FIX stronger semantic
  assertions; sameauthor resumed coverage/unknown/zero/free/grid/legend fixes.
- Tool projection original evaluator APPROVED18dd20ff... pairing/security only ->
  `ses_e8841025affeDXP37IKyoK14fU` (`replenish20-74-toolready`) private applyproof.
- Subagent fresh-frame supplemental152ebbc6... -> original evaluator
  `ses_e88476509ffevG4jiAnm7Bm2zY` v2 composition and assertions review.
No source-only or fixture-definition handoff counted as installed acceptance;
each completed assignment replenished promptly, no route token balance invented.

### Latest replenishment / verification queue

- Render actualfixture3b2c... reviewerNEEDS_FIX noHome/assertions/wiring; author
  delivered1232faf2... atallthreewidthswithHome/Chat/40rowassertions; original
  `ses_e88444b3affeqqCr514RFXpWNh` rereviewing exactv2.
- Compaction v5 sourceeditedafterpatchgeneration; staleac86...patchNOTcandidate.
  Optimizer `ses_e88bd254fffeg0P1UZYgSytZK9` resumed sourcefreeze/actualraceREDGREEN.
  HEAVYSLOT NOW OWNED BY THISWORKER, jobs1threads1, no mainparallelCargo.
- Deviceinstalledauth safelyBLOCKED pinnedrealOpenAIendpoints/nofixtureinjection;
  no real login performed. Replenished by permissionfixture
  `ses_e88402df5ffe3LIpcqIYjK57p2` (`replenish20-75-permissionjourney`), resumed
  write-first after initial noartifacthandoff. Savefieldcurrentlynottransported.
- Horizontal correctedfa273c2c... delivered -> independent evaluator
  `ses_e883fd8d7ffefJF0J4ZLqrV8vF` (`replenish20-76-horizontalreview`).
- Decoder v41 again incorrectly mapsAltUp8/AltDown9; no integration. Replacement
  `ses_e883fd8a8ffeRKeP8apTaujFKK` (`replenish20-77-decoderfix`) fixesactualKitty
  functionalcodes/consumerwiringproposal, oldcandidate277353...NOTapproved.
- Rail original reviewer reverseditsowndoubleoffsetrecommendation. Independent
  strategy `ses_e883ed3e8ffeH1STSStahLVr7o` (`replenish20-78-railstrategy`)
  resolvesactualrow/classindexinvariant. No rail patch integrated.
- Actualcontextpointermissing perfixture sourceinvestigation; implementer
  `ses_e883ed3e3ffeLI48tWqLyqDqaa` (`context-user-06-pointer`) producesstandalone
  nativeview/entryproposal; existingmetadata/Homeownersremainnonoverlapping.
- Keybind proposal8ef130d5... unvalidated -> evaluator
  `ses_e883ed3bdffeG1YzBeZJnrrGBZ` (`replenish20-79-keybindreview`).
- Approvedleaderproposal couldn'tapplybecauseexpectsunintroducedvariable ->
  optimizer `ses_e883ed3bbffe4lvfPIR7Kwek04` (`replenish20-80-leaderbase`)
  createscleanbasecandidate/newhashrequiringrenewedreview.
- Markdown coordinator independentlyapplied5897821a... against actualc772972...
  base, savedSEPARATEimmutablebasefile andfinal808855...; proofreceipt in69root.
  Originalevaluator nowAPPROVE exactpatch in v4note585ad8b5... . Focusedtests
  queued AFTERcompactionheavyslotrelease. Replacement assignment
  `ses_e883dd6edffeRvuou57Ch1z6Tu` (`replenish20-82-markdownjourney`) installed
  codeindent/Unicodefixture; no builds/PTY permission.
- Metadata corrected80997120... -> original `ses_e8857d060ffe9A8vfXrLsJ3ka7`
  exactv2 review. Auto stillunobserved; nofullmetadataparityclaim.
- Toolprojection applyproof candidate62ce0f30... ready at74-toolready;
  focusedtests queued AFTERcompactionheavyslotrelease. No product integration yet.
- Subagent freshdraft152ebbc6... -> originalreviewercomposing/reviewingactual
  revisedfixture, notdelta-onlyapproval.
- Composer reviewNEEDS_FIX unboundsession_id; authorcorrectedc33e9d8b...,
  original `ses_e8841d6b6ffebB3VQ6sK4kOO5a` v2rereview. No installedacceptance.
- Shell task ownership tentativeDropremoval BLOCKED; new boundedowner/lifecycle
  candidate `ses_e883e23cfffecFGExxuFOQAg0E` (`replenish20-81-shelllifecycle`).
  Keep pendingapprovaldisconnectcancel, daemonshutdownreap andexplicitcancel.
- Separate shellpreviewimplementation `ses_e883d66a4ffe0NEbKRNDrEzSPo`
  (`replenish20-83-shellpreview`) private native_shell grant, no execution.
- Context screenshotfixturesv2 a299d8f0... originalreviewerrereviewing; definition
  isnotconsumerexecution. Renderer/accountingstillprivatependingintegration.
- Livepermissioncandidateupdatedsourcebutstalepatch: authorresumed freeze/diff
  beforesecurityrereview; no replacingbrokerpolicyoraddingalwaysalloweligibility.
Replenishment remained continuous onnotifications; source HEAD advancedonlysingle
integrationwriter. All route token usage remainsUNKNOWN, no exhaustedrouteused.

Latest concrete handoffs and replacements:
- Context renderer private snapshot9c0eade9... frozen as native_context-review-snapshot.rs;
  independentreview `ses_e883b519fffeZ8jwTxEaIf77Z1` (`context-user-07-renderreview`).
  Mutableauthorresumed caller/painter proposal plusparsingtests, optionalusage_report
  anduncountedcategoriesexplicit. No compileddashboard or coloredUI claim yet.
- Context accounting authorresumed actualcapture hook with privatebaseextracted
  lib/state grant, NOTcanonicaldirtylib edits. Pointerworker independent standalone
  proposal. No categorydata derivedfromonly50messagepage.
- Context golden v3 actual677fac0a... originalreviewer finalboundedcontractreview.
  v1/v2reviewsnegativepreserved; no JSONdefinitionrepresentedasa testexecution.
- Permissionlive corrected0cf1e6c1.../c2ebc954... originalsecurityevaluator
  `ses_e88499c77ffeO60qQusdfYE6y4` reviewing v2, unknowneligibilityfailclosed.
- Keybind review NEEDS_FIX9dfd4bc8... actualKittyCtrlSpace/aliases/timer;
  optimizer `ses_e883b51b2ffe4MxJbsGJ2L5TDi` (`replenish20-86-keybindfix`).
- Subagent fixture missingchatcursorrevealsproductgap; implementer
  `ses_e883ac36dffeaFQF2d6ugK6oFt` (`replenish20-87-chatcursor`) preparesactual
  Chatcomposer cursorposition; do not weakenvalidcursorassertions intoPASS.
- Home actual82644966... -> evaluator `ses_e883ac366ffezx0lu3n2vkWO9O`
  (`replenish20-88-homereview`) newbaseb797diff, admission/draft/workerguards.
- Composer originalreviewer APPROVEDc33e9d8b... -> executionprep
  `ses_e883ac355ffedxi9pyNtDsE92M` (`replenish20-89-composerready`).
- Independentrailstrategy81849f59... proves subtracty_offset required; original
  reviewerfirstrecommendationwaswrong. Railauthorresumed correctoffset plus
  themePrimaryexplicitapproximation, stillnotupstreamagentcategoricalparity.
- Metadata80997120... negativeonlywidth43wrongtestarithmetic; authorresumed
  minimalexpectationcorrection withnewhash/rereviewrequired.
- Horizontal v41actual050858f2... mismatchedreportedhash; reviewerNEEDS_FIX
  openUUIDrefreshreopensclosedtabs andduplicateChatdoublecounting. Replacement
  optimizer `ses_e883c701cffel3th2K17NDypMO` (`replenish20-85-horizontalfix`).
- Render1232faf2... originalreviewerAPPROVED; overlayprep
  `ses_e883c7028ffeRPAEOi5oQeDlIx` (`replenish20-84-renderready`).
No oldapproval transfers to correctedhashes; no silentnegativeoutvotes. Every
completion inthissectionwasimmediatelyreplaced byusefulindependentassignment.

### Most recent approved candidates and replenishment

- Context golden v3 `677fac0a...` original evaluator APPROVED contract definition
  only (review946e2e76...). Replaced by actual RUST predicate-consumer implementer
  `ses_e88382b99ffeqIXnsQTYz3Yl40` (`context-user-08-goldenconsumer`), must call
  real ContextView, not validate fixture against itself. No rendered acceptance.
- Context mutable renderer9c0eade9... snapshot under independentreview07; author
  adding actual painter/caller diff. Pointer author resumed write-first actual
  geometry proposal after no-artifacthandoff. Accounting capture stillpending.
- Livepermission c2ebc954... SECURITYAPPROVE received (62627f94...), candidate
  preparation `ses_e88387ba7ffeOK5yac7LuK5Kf2` (`replenish20-92-permissionready`).
- Render1232faf2... approved/wired overlay88244585... candidatea476d273... ready
  for main's serialized NativeView Home/Chat capture test after compaction slot.
  Comparator implementer `ses_e88387bafffeF985K359Co3QJB`
  (`replenish20-91-renderdiff`) producesactualgridcomparison, colorsstillUNVERIFIED.
- Vertical bafe8c2d... stillopenUUIDmodelincomplete -> replacement optimizer
  `ses_e88387bb9ffei5Po74YKbDn9HG` (`replenish20-90-verticalidentity`), no accepted
  titlecount-only identity. v41noartifact/researchloops notcontinued unnecessarily.
- Bounded shellowner modulef84f3a0b... UNREADY -> independent SECURITY/async
  evaluator `ses_e88382b89ffe7ejMR1ZZiq7j9o` (`replenish20-93-shellsecurity`).
  Production runtime/shutdown wiring remains absent, no persistentPTYclaim.
- Canonicalleader04355ff1... nowprivateapplyproofpassed -> original evaluator
  `ses_e884e86d6ffeGgeYT6l2prwmnJ` v3review. Oldbadcontextapprovalnottransferred.
- Metadata60a1d1aa... originalAPPROVE (reviewfc7e95d7...), candidateprep
  `ses_e8837eb69ffeBbIZzzvPYdR5bY` (`replenish20-94-metadataready`), tests await
  serializedslot. No auto state yet; no full metadata parity claim.
All completion notifications replaced promptly. Canonical product HEAD remains
b797acda (documentation may advance); no release installation completed.
