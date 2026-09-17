# Example corpus

This catalog is a map of the Fidryn example tree. The modules under
`examples/` and the independent programs under `tests/programs/` are
research fixtures. They are not legal advice, not operative instruments,
and not a complete statement of any jurisdiction's law.

Build the toolchain as in [getting-started](getting-started.md). Source
syntax is in [language](language.md). Subcommands, timestamps, and
outcome envelopes are in [cli](cli.md). Commands below assume the
repository root as the working directory. The binary name is `fidryn`.
From a checkout that has not installed the binary, prefix the same
subcommands with `cargo run -p fidryn-cli --`.

## How to use an example

Every module is a `.fr` file. Start by checking it. `check` authenticates
the declared source manifest (when the module has one), elaborates, and
type-checks. A clean module prints `ok`.

```
fidryn check examples/trust/bryan-revocable-trust.fr
```

If the directory contains a `cases/` folder, those files are
`fidryn.case-record/v0.1` JSON records. `run` evaluates one named query
against one record at a pair of ISO 8601 / RFC 3339 timestamps. `run`
never chooses a completion. A `Suspended` outcome is an honest result,
not a failed command.

```
fidryn run examples/us-federal/flsa/flsa.fr \
  --query overtime_hours \
  --case examples/us-federal/flsa/cases/forty-one-hours.json \
  --valid-at 2026-09-17T12:00:00-04:00 \
  --known-at 2026-09-17T12:00:00-04:00
```

`--valid-at` is the legal valid time. `--known-at` is the record time.
Evidence whose `observedAt` is after `--known-at` does not discharge a
query. `--arg KEY=VALUE` writes `case.facts["KEY"]` on top of the JSON
(for example `--arg provision=ChildSupportWaiver`).

State modules under `examples/states/` do not ship companion JSON. Case
facts live in `scenario` blocks inside the `.fr` file. There is no JSON
source manifest either: the `source { ... }` block already carries
citation, artifact, and `digest fixture`. Check the module, and verify a
declared property when you want the honesty assertions:

```
fidryn check examples/states/florida/homestead.fr
fidryn verify examples/states/florida/homestead.fr --property HomesteadHonesty
```

A query that looks mechanical (deadlines, dollar caps, day counts,
acreage) is `automatic` or `Evaluate` with an empty effect row. A query
that needs a tribunal, agency, or open-textured standard uses `Determine`
or `Observe` and must be allowed to `Suspend`. The fixtures do not hide
judgment in a Boolean.

## Start here

These eleven modules cover an instrument, a closed-form tax slice, an
agency procedure, a formation filing, a family agreement, a civil-procedure
spine, a consumer clock, a state constitutional cap, a federal wage rule,
and two interpreter programs.

| Path | Purpose | Command |
| --- | --- | --- |
| `examples/trust/bryan-revocable-trust.fr` | Successor trustee occupancy under the Bryan revocable-trust fixture. | `fidryn check examples/trust/bryan-revocable-trust.fr` |
| `examples/tax/federal-tax.fr` | Closed-form ordinary-income tax and FinCEN BOI after the domestic-company exemption. | `fidryn check examples/tax/federal-tax.fr` |
| `examples/foia/foia-request.fr` | FOIA request processing; a proposed Exemption 5 withholding still needs harm and segregability. | `fidryn check examples/foia/foia-request.fr` |
| `examples/massachusetts-llc/harbor-robotics.fr` | Massachusetts LLC formation; a transport receipt is not an official filing record. | `fidryn check examples/massachusetts-llc/harbor-robotics.fr` |
| `examples/prenup/ava-noah.fr` | California UPAA provision enforcement for the Ava–Noah agreement. | `fidryn check examples/prenup/ava-noah.fr` |
| `examples/procedure/civil-complaint.fr` | FRCP 3/4/12/58 complaint-to-judgment spine. | `fidryn check examples/procedure/civil-complaint.fr` |
| `examples/california/security-deposit/security-deposit.fr` | California 21-day security-deposit return clock versus bad-faith doubling. | `fidryn check examples/california/security-deposit/security-deposit.fr` |
| `examples/states/florida/homestead.fr` | Florida homestead acreage caps and forced-sale character; scenarios live in the `.fr` file. | `fidryn check examples/states/florida/homestead.fr` |
| `examples/us-federal/flsa/flsa.fr` | FLSA 40-hour overtime versus the white-collar exemption. | `fidryn check examples/us-federal/flsa/flsa.fr` |
| `tests/programs/require-gate.fr` | `require true` returns 7; `require false` does not run the return. | `fidryn check tests/programs/require-gate.fr` |
| `tests/programs/late-payment.fr` | Invoice duty, timely-payment decision, and a strict-versus-extended deadline family. | `fidryn check tests/programs/late-payment.fr` |

Run the modules that ship JSON case records. Timestamps match each
module's `recorded_at` or, when evidence is later, that evidence's
`observedAt`.

```
fidryn run examples/trust/bryan-revocable-trust.fr \
  --query acting_trustee \
  --case examples/trust/cases/one-certificate.json \
  --valid-at 2026-08-23T12:00:00Z \
  --known-at 2026-08-23T12:00:00Z
```

Other trust records in the same folder are
`bob-plus-open-alice.json`, `two-certificates-open-eligibility.json`, and
`court-selects-i2.json`. One physician certificate without a checked
completion is expected to suspend.

```
fidryn run examples/tax/federal-tax.fr \
  --query tax_on \
  --case examples/tax/cases/ordinary-income.json \
  --valid-at 2026-08-14T00:00:00-04:00 \
  --known-at 2026-08-14T00:00:00-04:00
```

```
fidryn run examples/tax/federal-tax.fr \
  --query boi_required \
  --case examples/tax/cases/domestic-company-after-exemption.json \
  --valid-at 2026-09-01T00:00:00-04:00 \
  --known-at 2026-09-01T00:00:00-04:00
```

```
fidryn run examples/foia/foia-request.fr \
  --query disposition \
  --case examples/foia/cases/proposed-exemption-5-withholding.json \
  --valid-at 2026-08-23T12:00:00-04:00 \
  --known-at 2026-08-23T12:00:00-04:00
```

```
fidryn run examples/massachusetts-llc/harbor-robotics.fr \
  --query entity_status \
  --case examples/massachusetts-llc/cases/official-filing-record.json \
  --valid-at 2026-08-28T14:12:00-04:00 \
  --known-at 2026-08-28T14:12:00-04:00
```

`examples/massachusetts-llc/cases/transmitted-without-official-record.json`
is the contrast case: transmission without an `OfficialFilingRecord`
suspends.

```
fidryn run examples/prenup/ava-noah.fr \
  --query provision_result \
  --case examples/prenup/cases/divorce-record.json \
  --valid-at 2026-08-23T12:00:00-07:00 \
  --known-at 2026-08-23T12:00:00-07:00
```

`divorce-record-without-enforceability-order.json` suspends on
`PrenupEnforceability`. `--arg provision=ChildSupportWaiver` overwrites
`case.facts["provision"]` when you want a different clause without
editing the JSON.

```
fidryn run examples/procedure/civil-complaint.fr \
  --query judgment \
  --case examples/procedure/cases/judgment-entered.json \
  --valid-at 2026-10-01T16:00:00-04:00 \
  --known-at 2026-10-01T16:00:00-04:00
```

`complaint-answered-without-judgment.json` suspends because there is no
judgment entry.

```
fidryn run examples/california/security-deposit/security-deposit.fr \
  --query deadline_running \
  --case examples/california/security-deposit/cases/day-10.json \
  --valid-at 2026-09-17T12:00:00-07:00 \
  --known-at 2026-09-17T12:00:00-07:00
```

```
fidryn run examples/us-federal/flsa/flsa.fr \
  --query overtime_hours \
  --case examples/us-federal/flsa/cases/forty-one-hours.json \
  --valid-at 2026-09-17T12:00:00-04:00 \
  --known-at 2026-09-17T12:00:00-04:00
```

Florida homestead has no JSON case file. The scenarios `InsideCityHalfAcre`,
`RuralHundredSixty`, and `TaxCreditor` are in
`examples/states/florida/homestead.fr`. `require-gate.fr` and
`late-payment.fr` also have no JSON; they are check-only programs. See
[Independent programs](#independent-programs).

## Federal slices

Bounded United States Code and CFR slices live under
`examples/us-federal/`. Fidryn does not contain the United States Code.
The catalog module at `examples/us-federal/catalog/usc-titles.fr` records
that fact. Each slice names the dated source it encodes and lists what it
leaves `outside_scope`.

Unless a row says otherwise, these modules use the 2026-09-17 snapshot
and this timestamp pair:

```
--valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

Check, then run:

```
fidryn check PATH
fidryn run PATH --query QUERY --case CASE \
  --valid-at 2026-09-17T12:00:00-04:00 \
  --known-at 2026-09-17T12:00:00-04:00
```

The original FOIA spine is `examples/foia/foia-request.fr`, not under
`us-federal/`. It is in [Start here](#start-here).

### Litigators, every week

| Path | Purpose | Query | Case |
| --- | --- | --- | --- |
| `examples/us-federal/diversity/diversity.fr` | 28 U.S.C. § 1332(a). The $75,000 amount-in-controversy floor is mechanical; complete diversity is not. | `amount_in_controversy_met` | `examples/us-federal/diversity/cases/amount-met-diversity-open.json` |
| `examples/us-federal/removal/removal.fr` | 28 U.S.C. § 1446(b). Thirty-day removal clock versus later-served-defendant doctrine. | `thirty_day_window` | `examples/us-federal/removal/cases/inside-thirty-days.json` |
| `examples/us-federal/hearsay/hearsay.fr` | Fed. R. Evid. 801–804. Out-of-court use versus the 803(6) business-records exception. | `out_of_court` | `examples/us-federal/hearsay/cases/out-of-court-statement.json` |
| `examples/us-federal/speedy-trial/speedy-trial.fr` | 18 U.S.C. § 3161. Seventy-day clock versus ends-of-justice continuance. | `seventy_day_window` | `examples/us-federal/speedy-trial/cases/inside-seventy-days.json` |

```
fidryn run examples/us-federal/diversity/diversity.fr --query amount_in_controversy_met --case examples/us-federal/diversity/cases/amount-met-diversity-open.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/removal/removal.fr --query thirty_day_window --case examples/us-federal/removal/cases/inside-thirty-days.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/hearsay/hearsay.fr --query out_of_court --case examples/us-federal/hearsay/cases/out-of-court-statement.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/speedy-trial/speedy-trial.fr --query seventy_day_window --case examples/us-federal/speedy-trial/cases/inside-seventy-days.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

### Employment counsel and workers

| Path | Purpose | Query | Case |
| --- | --- | --- | --- |
| `examples/us-federal/flsa/flsa.fr` | 29 U.S.C. § 207. Hours over 40 are mechanical; the white-collar exemption is a determination. | `overtime_hours` | `examples/us-federal/flsa/cases/forty-one-hours.json` |
| `examples/us-federal/fmla/fmla.fr` | 29 U.S.C. § 2611. 1,250 hours / 12 months / 50 employees versus serious health condition. | `eligibility_counts` | `examples/us-federal/fmla/cases/hours-and-headcount.json` |
| `examples/us-federal/title-vii/title-vii.fr` | 42 U.S.C. § 2000e-5. 180/300-day EEOC charge clock versus equitable tolling. | `charge_clock` | `examples/us-federal/title-vii/cases/day-100-worksharing.json` |
| `examples/us-federal/warn/warn.fr` | 29 U.S.C. § 2102. Sixty-day / 100-employee floor versus unforeseen circumstances. | `notice_timely` | `examples/us-federal/warn/cases/sixty-day-notice.json` |

```
fidryn run examples/us-federal/flsa/flsa.fr --query overtime_hours --case examples/us-federal/flsa/cases/forty-one-hours.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/fmla/fmla.fr --query eligibility_counts --case examples/us-federal/fmla/cases/hours-and-headcount.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/title-vii/title-vii.fr --query charge_clock --case examples/us-federal/title-vii/cases/day-100-worksharing.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/warn/warn.fr --query notice_timely --case examples/us-federal/warn/cases/sixty-day-notice.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

California noncompete, final pay, and the ABC test are not under
`us-federal/`. They are in [Other practice slices](#other-practice-slices).

### Consumer, housing, and privacy

| Path | Purpose | Query | Case |
| --- | --- | --- | --- |
| `examples/us-federal/fcra/fcra.fr` | 15 U.S.C. § 1681i. Thirty-day credit reinvestigation clock. | `reinvestigation_clock` | `examples/us-federal/fcra/cases/day-10.json` |
| `examples/us-federal/fdcpa/fdcpa.fr` | 15 U.S.C. § 1692g. Thirty-day debt-validation clock. | `validation_clock` | `examples/us-federal/fdcpa/cases/day-10.json` |
| `examples/us-federal/efta/efta.fr` | 15 U.S.C. § 1693f. Sixty-day bank-error clock. | `error_clock` | `examples/us-federal/efta/cases/day-30.json` |
| `examples/us-federal/tcpa/tcpa.fr` | 47 U.S.C. § 227. Statutory-damages floor; consent and autodialer stay open-textured. | `at_least_statutory_floor` | `examples/us-federal/tcpa/cases/statutory-floor.json` |
| `examples/us-federal/dmca/dmca.fr` | 17 U.S.C. § 512. Six notice elements versus counter-notice good faith. | `notice_elements_met` | `examples/us-federal/dmca/cases/six-elements.json` |
| `examples/us-federal/ada-service-animal/ada-service-animal.fr` | 28 C.F.R. § 36.302. Two permitted questions versus task-training. | `two_questions_only` | `examples/us-federal/ada-service-animal/cases/two-questions.json` |
| `examples/us-federal/fair-housing/fair-housing.fr` | HUD FHEO-2020-01. Extra deposit is mechanical; disability-and-nexus is not. | `extra_deposit_not_required` | `examples/us-federal/fair-housing/cases/no-extra-deposit.json` |
| `examples/us-federal/garnishment/garnishment.fr` | 15 U.S.C. § 1673. Twenty-five percent disposable-earnings cap. | `within_twenty_five_percent` | `examples/us-federal/garnishment/cases/under-cap.json` |

```
fidryn run examples/us-federal/fcra/fcra.fr --query reinvestigation_clock --case examples/us-federal/fcra/cases/day-10.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/fdcpa/fdcpa.fr --query validation_clock --case examples/us-federal/fdcpa/cases/day-10.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/efta/efta.fr --query error_clock --case examples/us-federal/efta/cases/day-30.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/tcpa/tcpa.fr --query at_least_statutory_floor --case examples/us-federal/tcpa/cases/statutory-floor.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/dmca/dmca.fr --query notice_elements_met --case examples/us-federal/dmca/cases/six-elements.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/ada-service-animal/ada-service-animal.fr --query two_questions_only --case examples/us-federal/ada-service-animal/cases/two-questions.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/fair-housing/fair-housing.fr --query extra_deposit_not_required --case examples/us-federal/fair-housing/cases/no-extra-deposit.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/garnishment/garnishment.fr --query within_twenty_five_percent --case examples/us-federal/garnishment/cases/under-cap.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

### Benefits, health, military, and education

| Path | Purpose | Query | Case |
| --- | --- | --- | --- |
| `examples/us-federal/medicare/medicare.fr` | 42 U.S.C. § 1395p. Age-65 attainment versus initial-enrollment and late-enrollment penalty. | `age_65_attained` | `examples/us-federal/medicare/cases/turning-65.json` |
| `examples/us-federal/ssa-fra/ssa-fra.fr` | 42 U.S.C. § 416(l). Full retirement age 67 for birth year 1960 versus delayed credits. | `fra_is_sixty_seven` | `examples/us-federal/ssa-fra/cases/born-1960.json` |
| `examples/us-federal/ssi/ssi.fr` | 42 U.S.C. § 1382. $2,000 resource cap versus household-of-another. | `resources_within_cap` | `examples/us-federal/ssi/cases/under-cap.json` |
| `examples/us-federal/cobra/cobra.fr` | 29 U.S.C. § 1165. Sixty-day election window versus qualifying event. | `election_open` | `examples/us-federal/cobra/cases/day-30.json` |
| `examples/us-federal/hipaa/hipaa.fr` | 45 C.F.R. § 164.524. Thirty-day records clock versus psychotherapy notes. | `access_clock_running` | `examples/us-federal/hipaa/cases/day-10.json` |
| `examples/us-federal/scra/scra.fr` | 50 U.S.C. § 3937. Six-percent interest cap during military service. | `within_six_percent` | `examples/us-federal/scra/cases/six-percent.json` |
| `examples/us-federal/userra/userra.fr` | 38 U.S.C. § 4312. Reemployment window versus escalator position. | `within_outer_window` | `examples/us-federal/userra/cases/day-30.json` |
| `examples/us-federal/pslf/pslf.fr` | 20 U.S.C. § 1087e. 120 qualifying payments versus qualifying employer. | `payment_count_met` | `examples/us-federal/pslf/cases/one-hundred-twenty.json` |

```
fidryn run examples/us-federal/medicare/medicare.fr --query age_65_attained --case examples/us-federal/medicare/cases/turning-65.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/ssa-fra/ssa-fra.fr --query fra_is_sixty_seven --case examples/us-federal/ssa-fra/cases/born-1960.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/ssi/ssi.fr --query resources_within_cap --case examples/us-federal/ssi/cases/under-cap.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/cobra/cobra.fr --query election_open --case examples/us-federal/cobra/cases/day-30.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/hipaa/hipaa.fr --query access_clock_running --case examples/us-federal/hipaa/cases/day-10.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/scra/scra.fr --query within_six_percent --case examples/us-federal/scra/cases/six-percent.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/userra/userra.fr --query within_outer_window --case examples/us-federal/userra/cases/day-30.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/pslf/pslf.fr --query payment_count_met --case examples/us-federal/pslf/cases/one-hundred-twenty.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

### Immigration, bankruptcy, tax, and securities

| Path | Purpose | Query | Case |
| --- | --- | --- | --- |
| `examples/us-federal/naturalization/naturalization.fr` | 8 U.S.C. § 1427. Five-year / three-year residency versus good moral character. | `residency_met` | `examples/us-federal/naturalization/cases/five-years.json` |
| `examples/us-federal/unlawful-presence/unlawful-presence.fr` | 8 U.S.C. § 1182(a)(9)(B). 180-day / 1-year triggers versus waiver. | `three_year_bar_triggered` | `examples/us-federal/unlawful-presence/cases/one-hundred-eighty-days.json` |
| `examples/us-federal/bankruptcy-stay/bankruptcy-stay.fr` | 11 U.S.C. § 362. Stay on filing versus stay relief. | `stay_in_force` | `examples/us-federal/bankruptcy-stay/cases/day-after-filing.json` |
| `examples/us-federal/means-test/means-test.fr` | 11 U.S.C. § 707(b). Median-income comparison versus special circumstances. | `above_median` | `examples/us-federal/means-test/cases/above-median.json` |
| `examples/us-federal/eighty-three-b/eighty-three-b.fr` | 26 U.S.C. § 83(b). Thirty-day election window versus substantial risk of forfeiture. | `election_window_open` | `examples/us-federal/eighty-three-b/cases/day-10.json` |
| `examples/us-federal/accredited-investor/accredited-investor.fr` | 17 C.F.R. § 230.501. Income and net-worth tests versus sophistication. | `individual_income_met` | `examples/us-federal/accredited-investor/cases/income-met.json` |

```
fidryn run examples/us-federal/naturalization/naturalization.fr --query residency_met --case examples/us-federal/naturalization/cases/five-years.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/unlawful-presence/unlawful-presence.fr --query three_year_bar_triggered --case examples/us-federal/unlawful-presence/cases/one-hundred-eighty-days.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/bankruptcy-stay/bankruptcy-stay.fr --query stay_in_force --case examples/us-federal/bankruptcy-stay/cases/day-after-filing.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/means-test/means-test.fr --query above_median --case examples/us-federal/means-test/cases/above-median.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/eighty-three-b/eighty-three-b.fr --query election_window_open --case examples/us-federal/eighty-three-b/cases/day-10.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/us-federal/accredited-investor/accredited-investor.fr --query individual_income_met --case examples/us-federal/accredited-investor/cases/income-met.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

Closed-form IRC § 1 and FinCEN BOI live in `examples/tax/federal-tax.fr`,
not under `us-federal/`. See [Start here](#start-here).

### Catalog

`examples/us-federal/catalog/usc-titles.fr` (`US.Federal.Code.Catalog`)
does not encode titles. It records that the United States Code, the CFR,
uncodified statutes, case law, and internal guidance remain outside this
corpus except for the slices listed above.

```
fidryn check examples/us-federal/catalog/usc-titles.fr
fidryn run examples/us-federal/catalog/usc-titles.fr --query encoding_is_bounded --case examples/us-federal/catalog/cases/catalog-query.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
fidryn verify examples/us-federal/catalog/usc-titles.fr --property CatalogDoesNotClaimCompleteness
```

## Other practice slices

These directories sit beside `us-federal/` and `states/`. They still use
`fidryn.case-record/v0.1` JSON. Unless a command uses a Pacific or
Central offset from the module header, use
`--valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00`.

### California

| Path | Purpose | Query | Case |
| --- | --- | --- | --- |
| `examples/california/security-deposit/security-deposit.fr` | Cal. Civ. Code § 1950.5. Twenty-one-day return versus bad-faith doubling. | `deadline_running` | `examples/california/security-deposit/cases/day-10.json` |
| `examples/california/final-pay/final-pay.fr` | Cal. Lab. Code §§ 201–203. Immediate pay on discharge versus waiting-time penalty. | `pay_due_immediately` | `examples/california/final-pay/cases/discharged.json` |
| `examples/california/noncompete/noncompete.fr` | Cal. Bus. & Prof. Code § 16600 versus the sale-of-business exception. | `void_as_to_noncompete` | `examples/california/noncompete/cases/restraint-present.json` |
| `examples/california/abc-test/abc-test.fr` | Cal. Lab. Code § 2775. Prong A can be Boolean; prongs B and C cannot. | `prong_a_free_from_control` | `examples/california/abc-test/cases/prong-a-free.json` |
| `examples/california/lemon/lemon.fr` | Cal. Civ. Code § 1793.22. Repair attempts versus substantial impairment. | `reasonable_repair_attempts` | `examples/california/lemon/cases/four-attempts.json` |
| `examples/california/cpra/cpra.fr` | Cal. Civ. Code § 1798.130. Forty-five-day access clock. | `response_clock_running` | `examples/california/cpra/cases/day-20.json` |

```
fidryn run examples/california/security-deposit/security-deposit.fr --query deadline_running --case examples/california/security-deposit/cases/day-10.json --valid-at 2026-09-17T12:00:00-07:00 --known-at 2026-09-17T12:00:00-07:00
```

```
fidryn run examples/california/final-pay/final-pay.fr --query pay_due_immediately --case examples/california/final-pay/cases/discharged.json --valid-at 2026-09-17T12:00:00-07:00 --known-at 2026-09-17T12:00:00-07:00
```

```
fidryn run examples/california/noncompete/noncompete.fr --query void_as_to_noncompete --case examples/california/noncompete/cases/restraint-present.json --valid-at 2026-09-17T12:00:00-07:00 --known-at 2026-09-17T12:00:00-07:00
```

```
fidryn run examples/california/abc-test/abc-test.fr --query prong_a_free_from_control --case examples/california/abc-test/cases/prong-a-free.json --valid-at 2026-09-17T12:00:00-07:00 --known-at 2026-09-17T12:00:00-07:00
```

```
fidryn run examples/california/lemon/lemon.fr --query reasonable_repair_attempts --case examples/california/lemon/cases/four-attempts.json --valid-at 2026-09-17T12:00:00-07:00 --known-at 2026-09-17T12:00:00-07:00
```

```
fidryn run examples/california/cpra/cpra.fr --query response_clock_running --case examples/california/cpra/cases/day-20.json --valid-at 2026-09-17T12:00:00-07:00 --known-at 2026-09-17T12:00:00-07:00
```

Wage statements for California are the fifty-state module
`examples/states/california/wage-statements.fr`, not this directory.

### Delaware, Illinois, Massachusetts, New York, Texas, UCC, everyday

| Path | Purpose | Query | Case |
| --- | --- | --- | --- |
| `examples/delaware/dgcl-220/dgcl-220.fr` | 8 Del. C. § 220. Record stockholder is mechanical; proper purpose is not. | `stockholder_of_record` | `examples/delaware/dgcl-220/cases/record-stockholder.json` |
| `examples/illinois/bipa/bipa.fr` | 740 ILCS 14. Consent before biometrics versus willful damages. | `consent_before_biometric` | `examples/illinois/bipa/cases/no-consent.json` |
| `examples/massachusetts/security-deposit/security-deposit.fr` | M.G.L. c. 186 § 15B. Thirty-day itemization versus treble damages. | `itemization_deadline_running` | `examples/massachusetts/security-deposit/cases/day-15.json` |
| `examples/new-york/foil/foil.fr` | N.Y. Pub. Off. Law § 87. Five-day acknowledgment versus law-enforcement exemption. | `acknowledge_window_running` | `examples/new-york/foil/cases/day-3.json` |
| `examples/texas/homestead/homestead.fr` | Tex. Const. art. XVI § 50 and Tex. Prop. Code § 41.002. Urban acreage versus homestead character. | `urban_acreage_10_acres` | `examples/texas/homestead/cases/urban-eight-acres.json` |
| `examples/ucc/battle-of-forms/battle-of-forms.fr` | U.C.C. § 2-207. Additional terms between merchants versus material alteration. | `additional_term_between_merchants` | `examples/ucc/battle-of-forms/cases/both-merchants.json` |
| `examples/everyday/minimum-wage/minimum-wage.fr` | 29 U.S.C. § 206. Federal $7.25 floor; which local ordinance applies is not a Boolean. | `federal_floor` | `examples/everyday/minimum-wage/cases/federal-floor.json` |
| `examples/everyday/final-paycheck/final-paycheck.fr` | Default next payday versus California immediate pay. | `default_next_payday` | `examples/everyday/final-paycheck/cases/next-payday.json` |

```
fidryn run examples/delaware/dgcl-220/dgcl-220.fr --query stockholder_of_record --case examples/delaware/dgcl-220/cases/record-stockholder.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/illinois/bipa/bipa.fr --query consent_before_biometric --case examples/illinois/bipa/cases/no-consent.json --valid-at 2026-09-17T12:00:00-05:00 --known-at 2026-09-17T12:00:00-05:00
```

```
fidryn run examples/massachusetts/security-deposit/security-deposit.fr --query itemization_deadline_running --case examples/massachusetts/security-deposit/cases/day-15.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/new-york/foil/foil.fr --query acknowledge_window_running --case examples/new-york/foil/cases/day-3.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/texas/homestead/homestead.fr --query urban_acreage_10_acres --case examples/texas/homestead/cases/urban-eight-acres.json --valid-at 2026-09-17T12:00:00-05:00 --known-at 2026-09-17T12:00:00-05:00
```

```
fidryn run examples/ucc/battle-of-forms/battle-of-forms.fr --query additional_term_between_merchants --case examples/ucc/battle-of-forms/cases/both-merchants.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/everyday/minimum-wage/minimum-wage.fr --query federal_floor --case examples/everyday/minimum-wage/cases/federal-floor.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

```
fidryn run examples/everyday/final-paycheck/final-paycheck.fr --query default_next_payday --case examples/everyday/final-paycheck/cases/next-payday.json --valid-at 2026-09-17T12:00:00-04:00 --known-at 2026-09-17T12:00:00-04:00
```

The original instrument fixtures `examples/trust/`, `examples/prenup/`,
`examples/foia/`, `examples/massachusetts-llc/`, `examples/procedure/`,
and `examples/tax/` are in [Start here](#start-here).

## Fifty-state corpus

`examples/states/` has one full high-impact module per U.S. state:
homestead, final pay, security deposit, paid leave, no-fault, community
property, and similar rules people and lawyers actually hit. Each file
is a complete instrument (sources, entities, propositions, observations,
rules, duties, queries, scenarios, verification). These are research
fixtures, not legal advice, and not a complete statement of any state's
code.

Case facts are `scenario` blocks in the `.fr` file. There is no companion
JSON case file and no JSON source manifest. `source_manifest` is omitted
when the module's `source { ... }` block already carries citation,
artifact, and `digest fixture`; the compiler loads an empty manifest in
that case.

The quality bar is `examples/states/florida/homestead.fr`
(`Florida.Property.Homestead`, Fla. Const. art. X, § 4). Urban acreage
0.5, rural acreage 160, and the $1,000 personal-property cap are
`automatic`. Homestead character and excepted-creditor status stay in
`Determine` / `Observe`. Scenarios `InsideCityHalfAcre`,
`RuralHundredSixty`, and `TaxCreditor` sit in the same file. Fixture
excerpts live next to each module under `sources/` and are not official
editions.

```
fidryn check examples/states/florida/homestead.fr
fidryn verify examples/states/florida/homestead.fr --property HomesteadHonesty
```

District of Columbia and the territories are outside this corpus.

| State | File | Module | Principal citation |
| --- | --- | --- | --- |
| Alabama | `examples/states/alabama/final-wages.fr` | `Alabama.Employment.FinalWages` | Ala. Code §§ 8-24-2 to 8-24-3 |
| Alaska | `examples/states/alaska/security-deposit.fr` | `Alaska.Housing.SecurityDeposit` | AS 34.03.070 |
| Arizona | `examples/states/arizona/material-noncompliance.fr` | `Arizona.Housing.MaterialNoncomplianceNotice` | A.R.S. § 33-1368 |
| Arkansas | `examples/states/arkansas/homestead.fr` | `Arkansas.Property.Homestead` | Ark. Const. art. IX, §§ 3–5 |
| California | `examples/states/california/wage-statements.fr` | `California.Employment.WageStatements` | Cal. Lab. Code § 226 |
| Colorado | `examples/states/colorado/famli.fr` | `Colorado.Employment.FAMLI` | Colo. Rev. Stat. §§ 8-13.3-503 to 8-13.3-505 |
| Connecticut | `examples/states/connecticut/paid-sick-leave.fr` | `Connecticut.Employment.PaidSickLeave` | Conn. Gen. Stat. §§ 31-57r, 31-57s |
| Delaware | `examples/states/delaware/wage-payment.fr` | `Delaware.Employment.WagePayment` | 19 Del. C. §§ 1102, 1103 |
| Florida | `examples/states/florida/homestead.fr` | `Florida.Property.Homestead` | Fla. Const. art. X, § 4 |
| Georgia | `examples/states/georgia/dispossessory.fr` | `Georgia.Housing.Dispossessory` | O.C.G.A. § 44-7-50 et seq. |
| Hawaii | `examples/states/hawaii/security-deposit.fr` | `Hawaii.Housing.SecurityDeposit` | Haw. Rev. Stat. § 521-44 |
| Idaho | `examples/states/idaho/noncompete.fr` | `Idaho.Employment.Noncompete` | Idaho Code §§ 44-2701, 44-2704 |
| Illinois | `examples/states/illinois/wage-payment.fr` | `Illinois.Employment.WagePayment` | 820 ILCS 115/3 to 115/14 |
| Indiana | `examples/states/indiana/security-deposit.fr` | `Indiana.Housing.SecurityDeposit` | Ind. Code § 32-31-3 |
| Iowa | `examples/states/iowa/security-deposit.fr` | `Iowa.Housing.SecurityDeposit` | Iowa Code § 562A.12 |
| Kansas | `examples/states/kansas/homestead.fr` | `Kansas.Property.Homestead` | Kan. Const. art. 15, § 9 |
| Kentucky | `examples/states/kentucky/final-wages.fr` | `Kentucky.Employment.FinalWages` | KRS 337.055 |
| Louisiana | `examples/states/louisiana/community-property.fr` | `Louisiana.Family.CommunityProperty` | La. Civ. Code arts. 2338–2341 |
| Maine | `examples/states/maine/paid-family-leave.fr` | `Maine.Employment.PaidFamilyLeave` | 26 M.R.S. §§ 850-A to 850-J |
| Maryland | `examples/states/maryland/wage-payment.fr` | `Maryland.Employment.WagePayment` | Md. Code, Lab. & Empl. §§ 3-502, 3-505, 3-507.2 |
| Massachusetts | `examples/states/massachusetts/pfml.fr` | `Massachusetts.Employment.PFML` | M.G.L. c. 175M §§ 2–6 |
| Michigan | `examples/states/michigan/no-fault-pip.fr` | `Michigan.Insurance.NoFaultPIP` | MCL 500.3105, 500.3107 |
| Minnesota | `examples/states/minnesota/security-deposit.fr` | `Minnesota.Housing.SecurityDeposit` | Minn. Stat. § 504B.178 |
| Mississippi | `examples/states/mississippi/eviction.fr` | `Mississippi.Housing.Eviction` | Miss. Code Ann. §§ 89-8-13, 89-8-19, 89-8-39 |
| Missouri | `examples/states/missouri/security-deposit.fr` | `Missouri.Housing.SecurityDeposit` | Mo. Rev. Stat. § 535.300 |
| Montana | `examples/states/montana/wrongful-discharge.fr` | `Montana.Employment.WrongfulDischarge` | Mont. Code Ann. §§ 39-2-903 to 39-2-912 |
| Nebraska | `examples/states/nebraska/wage-payment.fr` | `Nebraska.Employment.WagePayment` | Neb. Rev. Stat. §§ 48-1229 to 48-1234 |
| Nevada | `examples/states/nevada/security-deposit.fr` | `Nevada.Housing.SecurityDeposit` | NRS 118A.240, 118A.242 |
| New Hampshire | `examples/states/new-hampshire/wage-payment.fr` | `NewHampshire.Employment.WagePayment` | N.H. Rev. Stat. Ann. §§ 275:43, 275:44 |
| New Jersey | `examples/states/new-jersey/security-deposit.fr` | `NewJersey.Housing.SecurityDeposit` | N.J.S.A. 46:8-21.1, 46:8-21.2 |
| New Mexico | `examples/states/new-mexico/owner-resident.fr` | `NewMexico.Housing.OwnerResident` | NMSA 1978 §§ 47-8-18, 47-8-20, 47-8-37 |
| New York | `examples/states/new-york/security-deposit-trust.fr` | `NewYork.Housing.SecurityDepositTrust` | N.Y. Gen. Oblig. Law §§ 7-103, 7-105 |
| North Carolina | `examples/states/north-carolina/security-deposit.fr` | `NorthCarolina.Housing.SecurityDeposit` | N.C.G.S. §§ 42-50 to 42-55 |
| North Dakota | `examples/states/north-dakota/homestead.fr` | `NorthDakota.Property.Homestead` | N.D. Const. art. XI, § 22 |
| Ohio | `examples/states/ohio/security-deposit.fr` | `Ohio.Housing.SecurityDeposit` | Ohio Rev. Code § 5321.16 |
| Oklahoma | `examples/states/oklahoma/wage-payment.fr` | `Oklahoma.Employment.WagePayment` | 40 O.S. §§ 165.2, 165.3 |
| Oregon | `examples/states/oregon/no-cause-notice.fr` | `Oregon.Housing.NoCauseNotice` | ORS 90.427 |
| Pennsylvania | `examples/states/pennsylvania/security-deposit.fr` | `Pennsylvania.Housing.SecurityDeposit` | 68 P.S. §§ 250.511a–250.512 |
| Rhode Island | `examples/states/rhode-island/security-deposit.fr` | `RhodeIsland.Housing.SecurityDeposit` | R.I. Gen. Laws §§ 34-18-19, 34-18-24 |
| South Carolina | `examples/states/south-carolina/residential-landlord.fr` | `SouthCarolina.Housing.ResidentialLandlord` | S.C. Code Ann. §§ 27-40-410, 27-40-710 |
| South Dakota | `examples/states/south-dakota/wage-payment.fr` | `SouthDakota.Employment.WagePayment` | S.D. Codified Laws §§ 60-11-9, 60-11-10 |
| Tennessee | `examples/states/tennessee/urlta.fr` | `Tennessee.Housing.URLTA` | Tenn. Code Ann. § 66-28-301 et seq. |
| Texas | `examples/states/texas/payday.fr` | `Texas.Employment.Payday` | Tex. Lab. Code ch. 61 |
| Utah | `examples/states/utah/eviction.fr` | `Utah.Housing.Eviction` | Utah Code §§ 78B-6-802, 78B-6-811 |
| Vermont | `examples/states/vermont/security-deposit.fr` | `Vermont.Housing.SecurityDeposit` | 9 V.S.A. §§ 4451, 4461 |
| Virginia | `examples/states/virginia/vrlta-deposit.fr` | `Virginia.Housing.VRLTADeposit` | Va. Code § 55.1-1226 |
| Washington | `examples/states/washington/paid-sick-leave.fr` | `Washington.Employment.PaidSickLeave` | RCW 49.46.210 |
| West Virginia | `examples/states/west-virginia/final-wages.fr` | `WestVirginia.Employment.FinalWages` | W. Va. Code § 21-5-4 |
| Wisconsin | `examples/states/wisconsin/security-deposit.fr` | `Wisconsin.Housing.SecurityDeposit` | Wis. Stat. § 704.28; ATCP 134.06 |
| Wyoming | `examples/states/wyoming/wage-payment.fr` | `Wyoming.Employment.WagePayment` | Wyo. Stat. § 27-4-104 |

Check any row the same way:

```
fidryn check examples/states/georgia/dispossessory.fr
fidryn check examples/states/louisiana/community-property.fr
fidryn check examples/states/michigan/no-fault-pip.fr
fidryn check examples/states/montana/wrongful-discharge.fr
fidryn check examples/states/oregon/no-cause-notice.fr
fidryn check examples/states/texas/payday.fr
```

## Independent programs

`tests/programs/` holds small modules that exercise the interpreter.
They are not jurisdiction slices. They declare `outside_scope`, they
have no `cases/` directory, and `fidryn run` cannot be aimed at them
without a case JSON you write yourself. Check them:

```
fidryn check tests/programs/eligibility-succession.fr
fidryn check tests/programs/named-decision.fr
fidryn check tests/programs/two-offices.fr
fidryn check tests/programs/require-gate.fr
fidryn check tests/programs/late-payment.fr
fidryn check tests/programs/late-payment-extended.fr
```

| Path | Purpose |
| --- | --- |
| `tests/programs/eligibility-succession.fr` | `UniqueOccupant` for `TrusteeOf(BRT)` under competing `SuccessorEligibility` interpretations. Query `acting_trustee`. |
| `tests/programs/named-decision.fr` | `RunDecision` on `ProcessResponsiveRecord`; the observation is required. Query `q`. |
| `tests/programs/two-offices.fr` | Two independent offices, trustee and executor, each with its own eligibility family. Queries `acting_trustee` and `acting_executor`. |
| `tests/programs/require-gate.fr` | Query `q` is `require true; return 7`. Query `r` is `require false; return 7`. False require does not run the return. |
| `tests/programs/late-payment.fr` | `PayInvoice` duty, `TimelyPayment` decision, and `DeadlineMeaning` (strict versus 15 counted days). Queries `due`, `paid_on_time`, `obligation_status`. |
| `tests/programs/late-payment-extended.fr` | Same duty shape with `due 15 counted_days after invoice_date` and no competing deadline family. |

## Honesty

These fixtures are not legal advice and not a complete code. Read each
module's `outside_scope` before treating an answer as even a research
result.

**`outside_scope`.** Every module names the topics, titles, doctrines,
and procedures it does not encode. Outcomes carry that boundary. An
omitted interpretation cannot silently shrink the model.

**`digest fixture`.** Source blocks on the 2026-09-17 slices use
`digest fixture`. That is a fixture trust profile, not byte-verified
authentication of the artifact, and not an official edition. Excerpts
under each module's `sources/` directory end by saying they are not
official editions.

**Not the United States Code.** The United States Code is a codification
of statutes, not the whole of federal law. Regulations, case law,
internal guidance, and uncodified provisions sit elsewhere. This corpus
encodes bounded high-impact slices. `US.Federal.Code.Catalog` exists so
the interpreter cannot honestly claim completeness.

`run` never chooses a completion. Open-textured queries must be allowed
to `Suspend`. The kernel does not search.
