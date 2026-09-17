# Fidryn example corpus

A browseable catalog with copy-paste `check` / `run` commands is
[docs/examples.md](../docs/examples.md).

These modules are research fixtures. They are not legal advice, not a
complete statement of any jurisdiction's law, and not operative
instruments. Every module names the dated source slice it encodes and
lists what it leaves `outside_scope`.

Fidryn cannot honestly claim to contain "all of federal law." The United
States Code is a codification of statutes, not the whole of federal law
(regulations, case law, internal guidance, and uncodified provisions sit
elsewhere). This corpus therefore encodes **bounded, high-impact
slices** and keeps a catalog of the rest as explicit outside scope.

## Who each slice is for

| Audience | Modules |
| --- | --- |
| Litigators, every week | diversity jurisdiction, removal clock, hearsay map, Speedy Trial Act |
| Transactional / corporate | DGCL 220 books and records, UCC 2-207 battle of forms, IRC 83(b) 30-day election |
| Employment counsel and workers | FLSA overtime, FMLA eligibility, Title VII charge deadline, WARN, CA noncompete, CA final pay, CA ABC test |
| Immigration | naturalization residency, unlawful-presence 3/10-year bars |
| Consumer / everyday | FCRA dispute, FDCPA validation, EFTA unauthorized transfer, TCPA, DMCA 512, lemon law, security deposit, minimum wage stack, Medicare enrollment, Social Security FRA, SSI resources, COBRA, HIPAA access, PSLF, garnishment caps, SCRA 6% |
| Housing | Fair Housing service animals, CA security deposit, NY FOIL for housing records, TX homestead |
| Privacy | CPRA request, BIPA, HIPAA |
| Bankruptcy | automatic stay, means test |
| Military / veterans | SCRA, USERRA |

## Encoding rule

A query that looks mechanical (deadlines, dollar caps, day counts) is
`automatic` or `Evaluate` with an empty effect row. A query that needs a
tribunal, agency, or open-textured standard uses `Determine` or
`Observe` and must be allowed to `Suspend`. Do not hide judgment in a
Boolean.

## Layout

```
examples/
  states/         one high-impact module per U.S. state (scenarios in .fr)
  us-federal/     bounded U.S. Code and CFR slices
  california/     high-volume California practice
  texas/          homestead
  illinois/       BIPA
  new-york/       FOIL
  massachusetts/  security deposit (pairs with the LLC fixture)
  delaware/       DGCL 220
  ucc/            Article 2 battle of forms
  everyday/       stacked minimum wage and similar person-facing tools
  trust, prenup, foia, massachusetts-llc, procedure, tax   (original fixtures)
```

The federal catalog is `us-federal/catalog/usc-titles.fr`. The fifty-state
index is `states/README.md`.

## Fifty states

`examples/states/` has one **full** high-impact module per state (homestead,
final pay, security deposit, paid leave, no-fault, community property, and
so on). Case facts are `scenario` blocks in the `.fr` file, not JSON.
See `examples/states/README.md`. Older federal and specialty fixtures still
use `fidryn.case-record/v0.1` JSON for CLI case-record interchange.

## Module index

All of these `fidryn check` clean. Each has fixture sources and at least one case record.

### Courts and evidence (lawyers)

- `US.Federal.Courts.Diversity` — 28 U.S.C. § 1332; $75,000 is mechanical, complete diversity is not
- `US.Federal.Courts.Removal` — 28 U.S.C. § 1446; 30-day clock vs later-served-defendant doctrine
- `US.Federal.Criminal.SpeedyTrial` — 18 U.S.C. § 3161; 70-day clock vs ends-of-justice continuance
- `US.Federal.Evidence.Hearsay` — FRE 801–804; out-of-court use vs 803(6) business records
- `Examples.CivilComplaint` — FRCP 3/4/12/58 spine

### Employment (lawyers and workers)

- `US.Federal.Labor.FLSAOvertime` — 40-hour overtime vs white-collar exemption
- `US.Federal.Labor.FMLAEligibility` — 1,250 hours / 12 months / 50 employees vs serious health condition
- `US.Federal.Labor.TitleVIIChargeDeadline` — 180/300-day EEOC clock vs equitable tolling
- `US.Federal.Labor.WARN` — 60-day / 100-employee floor vs unforeseen circumstances
- `California.Employment.Noncompete` — Bus. & Prof. § 16600 vs sale-of-business exception
- `California.Employment.FinalPay` — Lab. Code §§ 201–203 vs waiting-time penalty
- `California.Employment.ABCTest` — ABC test; prongs B and C are not Booleans
- `US.Everyday.MinimumWageStack` — FLSA $7.25 floor; higher state/city wage controls
- `US.Everyday.FinalPaycheckGuide` — default next payday vs California immediate pay

### Consumer, housing, privacy (everyday)

- `US.Federal.Consumer.FCRADispute` — 30-day credit reinvestigation
- `US.Federal.Consumer.FDCPAValidation` — 30-day debt validation
- `US.Federal.Consumer.EFTAUnauthorized` — 60-day bank-error clock
- `US.Federal.Consumer.TCPA` — robocall consent / autodialer
- `US.Federal.Copyright.DMCA512` — takedown elements vs counter-notice good faith
- `US.Federal.ADA.ServiceAnimal` — two permitted questions vs task-training
- `US.Federal.Housing.AssistanceAnimal` — HUD nexus is not a Boolean
- `California.Housing.SecurityDeposit` — 21-day return vs bad-faith doubling
- `Massachusetts.Housing.SecurityDeposit` — 30-day itemization vs treble damages
- `California.Consumer.LemonLaw` — repair attempts vs substantial impairment
- `California.Privacy.CPRARequest` — 45-day access clock
- `Illinois.Privacy.BIPA` — consent before biometrics vs willful damages
- `NewYork.Transparency.FOIL` — five-day acknowledgment vs law-enforcement exemption
- `Texas.Property.Homestead` — urban acreage vs homestead character
- `US.Federal.FOIA.RequestProcessing` — original FOIA spine

### Benefits, health, military, education

- `US.Federal.Health.MedicareEnrollment` — IEP vs late-enrollment penalty
- `US.Federal.Benefits.SocialSecurityFRA` — FRA 67 vs delayed credits
- `US.Federal.Benefits.SSIResources` — $2,000 cap vs household-of-another
- `US.Federal.Health.COBRAElection` — 60-day election vs qualifying event
- `US.Federal.Health.HIPAAAccess` — 30-day records vs psychotherapy notes
- `US.Federal.Military.SCRAInterest` — 6% cap
- `US.Federal.Military.USERRA` — reemployment window vs escalator position
- `US.Federal.Education.PSLF` — 120 payments vs qualifying employer
- `US.Federal.Garnishment.CCPA` — 25% disposable-earnings cap

### Immigration, bankruptcy, tax, corporate

- `US.Federal.Immigration.NaturalizationResidency` — 5-year / 3-year vs good moral character
- `US.Federal.Immigration.UnlawfulPresenceBars` — 180-day / 1-year triggers vs waiver
- `US.Federal.Bankruptcy.AutomaticStay` — stay on filing vs stay relief
- `US.Federal.Bankruptcy.MeansTest` — median income vs special circumstances
- `US.Federal.Tax.Section83b` — 30-day 83(b) election vs substantial risk of forfeiture
- `US.Federal.Tax.ClosedForms` — ordinary-income formula and FinCEN BOI
- `US.Federal.Securities.AccreditedInvestor` — income/net-worth tests vs sophistication
- `Delaware.DGCL.BooksAndRecords` — 8 Del. C. § 220 proper purpose
- `US.Commercial.UCC.BattleOfForms` — UCC 2-207 material alteration
- `Examples.FormHarborRoboticsLLC` — Massachusetts LLC formation
- `Examples.BryanRevocableTrust` / `Examples.AvaNoahPrenup` — original instruments

### Fifty-state high-impact slices

One module per state under `states/<slug>/`. Principal citations and
file names are in `states/README.md`. Examples:

- `Florida.Property.Homestead` — Fla. Const. art. X, § 4; 0.5 urban / 160 rural acres
- `California.Employment.WageStatements` — Lab. Code § 226 nine-item statement vs knowing-and-intentional
- `Texas.Employment.Payday` — Lab. Code ch. 61 payday clock vs good-faith dispute
- `Louisiana.Family.CommunityProperty` — Civ. Code arts. 2338–2341 classification vs determination
- `Michigan.Insurance.NoFaultPIP` — MCL 500.3105 / 3107 allowable expenses vs arising-out-of-use
- `Georgia.Housing.Dispossessory` — O.C.G.A. § 44-7-50 three-working-day pay-or-vacate vs possession
- `Montana.Employment.WrongfulDischarge` — WDEA 12-month probation vs good-cause determination
- `Oregon.Housing.NoCauseNotice` — ORS 90.427 30/90-day clocks vs qualifying-landlord-reason

### Catalog

- `US.Federal.Code.Catalog` — records that the United States Code is not fully encoded

