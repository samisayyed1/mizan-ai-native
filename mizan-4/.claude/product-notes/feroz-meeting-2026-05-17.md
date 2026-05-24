# Feroz Meeting — Product Direction (2026-05-17)

> **Source of truth** for the Mizan product overhaul Sami agreed to with
> Uncle Feroz on May 17, 2026 at 11:58 EDT. Every architecture/UX
> decision in this document is binding for the next-Sunday review and
> the soft-launch milestone. When in doubt, this file wins.
>
> Companion files: the call transcript is preserved verbatim at the
> bottom of this document. The Excel sheet Feroz mentioned is **pending
> from him** — when it lands, drop it under `.claude/product-notes/`
> next to this file.

---

## TL;DR — Old vs new product shape

**Old (today):**
```
Dashboard
  → Accounts
      → Holdings (mixed stock-only view)
```

**New (post-meeting):**
```
Dashboard
  → Overall net worth, consolidated graph, goals, portfolios
Portfolio (US, SGX, Moomoo, …)
  → Portfolio currency + consolidated portfolio graph
  → Asset Classes (Stocks, Sukuks, ETFs, Bonds, Bank Accounts, Property, Collectibles, …)
      → Holdings (only visible after picking an asset class)
          → Individual asset/account with amount + currency
Net Worth = Assets − Liabilities
  Assets: portfolios + property + collectibles + bank cash + (manual rental income)
  Excluded: vehicles / depreciating assets
Goals
  → Standard (retirement / education / home / savings / wedding) + Custom
  → Linkable to one-or-many portfolios
Liabilities
  → Auto loans, credit-card loans, mortgages, other
  → Fields: type, current balance, balance date, origination date,
    loan duration, optional interest %; EMI is the monthly payment,
    NOT the liability itself
```

---

## Binding decisions (numbered for traceability)

### Dashboard
1. **Rename "Accounts" → "Portfolio"** everywhere.
2. **Remove holdings from the main dashboard.** Holdings only appear
   after the user has drilled into an asset class. Reason: mixing
   stocks-only into the dashboard makes the user wonder "what about my
   sukuks?"
3. **Dashboard may show:** overall net worth, consolidated graph,
   goals, portfolio cards.
4. **Consolidated graph = sum of every portfolio's value.** Different
   asset classes render with different-colored lines if shown together;
   scale problems are resolved by drilling-in.

### Portfolio (the container)
5. A portfolio behaves like a **holding company** — it contains
   *asset classes*, not holdings directly.
6. **Multi-portfolio:** users can have any number (`US Portfolio`,
   `SGX Portfolio`, `Moomoo Portfolio`, `LSE Portfolio`, …).
7. **Per-portfolio currency:** USD for US Portfolio, SGD for SGX
   Portfolio, etc. Configurable at create / edit time.
8. **Portfolio detail page:** graph at top, list of asset classes below.

### Asset classes
9. **Replace the holdings list on the portfolio page with an asset
   class list.**
10. **Asset classes to support out of the gate:**
    - Stocks
    - Sukuks
    - ETFs
    - Bonds
    - Bank Accounts (new — see below)
    - Property
    - Collectibles
    - Other financial assets
11. **Vehicles are NOT an asset class** — they depreciate and would
    skew net worth (Feroz's "Rolls-Royce worth a million riyal but
    depreciating" example).
12. **Asset-class detail page:** filtered graph for that class only +
    list of holdings.
13. **Empty asset-class state:** big add CTA + offer manual-add /
    broker-API connect / CSV upload from the same screen. Do NOT push
    the user into Settings to add an asset.

### Holdings
14. Holdings live **inside** an asset class.
15. Empty-state copy examples Feroz gave: "Add Sukuk", "You don't have
    any sukuks. Please add now."
16. From the empty state, surface integrations when they exist:
    "Connect IBKR", "Connect Moomoo", "Upload CSV".

### Bank accounts (special-cased asset class)
17. **Bank Accounts is an asset class.** Examples: Axis Bank, ICICI
    Bank, DBS Bank are *holdings* under it.
18. **Each bank-account holding captures:**
    - Bank name
    - Country
    - Amount
    - Currency
19. **Country must NOT auto-lock currency.** A Saudi account can hold
    SAR + USD. A Dubai account can hold AED + USD. A DBS Singapore
    account can hold seven currencies. User picks the currency freely.
20. **Same bank, multiple currencies:** modeled as separate holdings
    under Bank Accounts. E.g. `DBS — USD` and `DBS — SGD` are two
    rows.
21. **Multiple bank accounts** per asset class is the default
    expectation.

### Currency
22. **Per-portfolio currency** (decision 7).
23. **Per-holding currency** (decision 18).
24. **Primary / master dashboard currency** lives in Settings. Default
    USD; user can switch to PKR, INR, SGD, AED, SAR, etc. All
    dashboard totals re-display in this currency; individual holdings
    keep their native currency.

### Property
25. New "is this property rented?" flag.
26. **Rental fields when flagged rented:**
    - Rental amount
    - Frequency (monthly / annual)
    - Rental start date (required)
    - Rental end date (**optional** — empty means perpetual)
27. **Rental income contributes to net worth.** Manually for now —
    don't try to dedupe against linked bank cash flow yet (avoids
    double-counting). Feroz: "Don't have to make it perfect at this
    stage."

### Collectibles
28. **Keep collectibles** as an asset category. Examples Feroz gave:
    watches, precious items.

### Goals
29. **Add a "Custom Goal" option** alongside Retirement / Education /
    Home / Savings / Wedding.
30. **Goals can link to one or more portfolios.** The goal dashboard
    shows linked-portfolio progress against the goal.

### Liabilities (new section — required for net worth)
31. **Liabilities reduce net worth.** Net Worth = Assets − Liabilities.
32. **Liability types to support:** Auto loan, Credit-card loan,
    Mortgage / Property loan, Other loan.
33. **Liability fields:**
    - Liability type
    - **Current balance** (the outstanding debt)
    - **Balance date** (when the current balance was measured)
    - **Origination date** = loan start date
    - **Loan duration** (years) — preferred over "end date" per Feroz
    - **Interest %** — optional
    - Notes
34. **EMI is NOT a liability.** EMI is the monthly installment the
    user pays. Liability = outstanding debt. Capture EMI as a separate
    "monthly payment" field if surfaced at all; do not let it reduce
    net worth on its own.

### Net Worth — final formula
35. **Assets** (counted):
    - Portfolios (all asset classes inside)
    - Bank account balances
    - Property current values
    - Collectibles
    - Rental income (manually applied for now)
36. **Excluded:** vehicles, any depreciating asset.
37. **Liabilities** (subtracted):
    - Auto loans, credit-card loans, mortgages, other loans.
38. **Formula:** `Net Worth = Σ Assets − Σ Liabilities`.

### Onboarding
39. Before soft launch, build a **guided onboarding walkthrough** with
    arrows / tooltips — similar to how Axis Bank introduces app
    updates. Should cover: dashboard layout, portfolio creation, asset
    classes, holdings, graphs, goals, net worth.

### Dummy data — Sunday deadline
40. Before the next Sunday meeting, populate dummy data covering:
    - Multiple portfolios (with different currencies)
    - Each asset class (stocks, sukuks, ETFs, bonds, bank accounts,
      property, collectibles)
    - Multi-currency bank-account holdings
    - At least one rented property + rental income
    - Multiple goals (standard + custom) linked to portfolios
    - Multiple liabilities (auto loan + credit-card + mortgage)
    - Verify dashboard graph, asset-class graphs, and net-worth math
      against the populated data.
41. **Feroz will send his Excel sheet.** When it arrives, align the
    dummy data shape to the spreadsheet so Sunday's review uses
    realistic numbers.

---

## Open questions / things NOT yet decided
- Whether broker-API auto-link for bank cash flow ships before Sunday —
  Feroz is OK with manual entry for now.
- Exact naming for the master currency setting ("Primary Currency" vs
  "Master Currency" — both used in the call; Sami to pick one).
- How custom-goal progress is calculated when multiple portfolios are
  linked — sum total value? specific asset classes? — needs follow-up.
- Whether collectibles need their own subcategories (watches /
  jewelry / art / other) or stay as a free-text "Collectibles" bucket.

---

## Implementation backlog (extracted, prioritized for Sunday)

Order roughly matches "biggest visible change first":

1. **Rename Accounts → Portfolio** (terminology pass across the app).
2. **Strip holdings from main dashboard**; replace with portfolio
   cards + consolidated graph + goals.
3. **Portfolio detail page redesign** — graph + asset-class list (not
   holdings).
4. **Asset-class detail page** — graph + holdings list + empty-state
   add CTA with manual / broker / CSV branches.
5. **Bank Accounts as a first-class asset class** — schema + UI +
   multi-currency support.
6. **Per-portfolio currency selection** at create / edit time.
7. **Primary dashboard currency setting** + conversion display layer.
8. **Remove Vehicle** from net-worth asset categories.
9. **Property rental fields** (rented flag, amount, frequency, start,
   optional end).
10. **Liabilities section** — schema + CRUD + dropdown of types + the
    five fields above + net-worth subtraction.
11. **Custom Goal** option + portfolio-linking on goals.
12. **Dummy data seeder** so the Sunday demo has realistic numbers.
13. **Onboarding walkthrough** — ship before soft launch (post-Sunday
    is fine, but Feroz wants direction visible).

---

## Verbatim transcript (preserved)

Meeting May 17, 2026 at 11:58 EDT. Participants: Sami Sayyed, Feroz
Siddiqui.

```
00:00:00
Sami Sayyed: happen. Nice. I'm taking notes as well. Okay, perfect. So, um let's go um page wise. You you can see the screen, right?
Feroz Siddiqui: Yeah. Yeah,
Sami Sayyed: Perfect. So,
Feroz Siddiqui: I can.
Sami Sayyed: this is the dashboard we have right now. Um what are the additions you want over here in the dashboard?
Feroz Siddiqui: So, first thing change accounts to
Sami Sayyed: Just Okay.
Feroz Siddiqui: portfolio.
Sami Sayyed: Change accounts to portfolio. Okay.
Feroz Siddiqui: Okay.
Sami Sayyed: Mhm.
Feroz Siddiqui: And then um instead of holdings here.
Sami Sayyed: Mhm.
Feroz Siddiqui: Okay. Uh, no. So, if you have only Okay. I don't think you should show the holdings here.
Sami Sayyed: Okay. Don't show holdings here.
Feroz Siddiqui: Yeah.
Sami Sayyed: Okay.
Feroz Siddiqui: No goals you can show.
Sami Sayyed: Okay. Goals we can
Feroz Siddiqui: Yeah. No,
Sami Sayyed: show.
Feroz Siddiqui: don't show the holdings here because imagine when you put in other stuff then why is it showing me only the stocks?

00:00:49
Feroz Siddiqui: What about my suks and everything else? You see it?
Sami Sayyed: Exactly. So it should show you multiple things.
Feroz Siddiqui: It can show me goals like which is which is manually uh edited by people and then under the
Sami Sayyed: Yes.
Feroz Siddiqui: portfolio the first one would be um US portfolio let's say for me in my case.
Sami Sayyed: Okay. US
Feroz Siddiqui: Yeah.
Sami Sayyed: portfolio.
Feroz Siddiqui: The other one is Singapore uh SGX portfolio.
Sami Sayyed: SGX
Feroz Siddiqui: Yeah.
Sami Sayyed: portfolio.
Feroz Siddiqui: Then I can choose the currency in US dollar. Here I can choose the currency Singapore dollars.
Sami Sayyed: Yes, you can do that. Okay.
Feroz Siddiqui: Yeah. So that way I can have different portfolios.
Sami Sayyed: See. Mhm.
Feroz Siddiqui: Then once I'm in the portfolio, uh how do I add my So how do I add the asset classes?
Sami Sayyed: Mhm.
Feroz Siddiqui: So uh once I have the portfolio in that portfolio, um keep the portfolio as a holding company like it has nothing in it.

00:01:43
Feroz Siddiqui: Right now what's happening is the portfolio has all the stocks in it directly.
Sami Sayyed: No.
Feroz Siddiqui: Har you need to hear me out.
Sami Sayyed: So
Feroz Siddiqui: So what's happening right now? You have um what do you call that?
Sami Sayyed: account
Feroz Siddiqui: Um uh u no um instead of account you have what? Uh portfolio.
Sami Sayyed: This portfolio
Feroz Siddiqui: Yeah. So instead of account you have portfolio.
Sami Sayyed: portfolio.
Feroz Siddiqui: When you click on portfolio number one. Okay. This new account I'm calling it portfolio number one.
Sami Sayyed: Okay. Okay.
Feroz Siddiqui: When I click on portfolio one. Okay.
Sami Sayyed: I
Feroz Siddiqui: Then click it. Yeah. Now here it should give me a screen where and don't give me the graph here now anymore.
Sami Sayyed: Okay.
Feroz Siddiqui: Okay,
Sami Sayyed: Perfect.
Feroz Siddiqui: you know give me give me uh uh yeah you can leave the graph actually I don't care about the graph so much but below don't give me holdings you should say asset classes.

00:02:32
Sami Sayyed: asset class. Perfect.
Feroz Siddiqui: Yeah so all asset classes should be listed there.
Sami Sayyed: Mhm.
Feroz Siddiqui: Yeah,
Sami Sayyed: Okay.
Feroz Siddiqui: stocks, you know, this,
Sami Sayyed: Understood.
Feroz Siddiqui: this, this, and people can have two stocks if they want to, you know, like one USD, one SGX, one LSC, whatever they want. So, they should be able to have multiple um, you know, portfolios because the reason is that some of my portfolio is on mumu. So, I create one for mumu separately. Yeah. But it's all under one portfolio.
Sami Sayyed: Okay.
Feroz Siddiqui: So that when my dashboard comes up, it gives me a overall picture of my portfolio.
Sami Sayyed: Okay. Overall
Feroz Siddiqui: So one assume that you can't make more portfolio first.
Sami Sayyed: picture.
Feroz Siddiqui: Okay? So that that structure will be a bit more clearer for you. So only one portfolio in that portfolio multiple asset
Sami Sayyed: Mhm.
Feroz Siddiqui: classes.
Sami Sayyed: Perfect. In one portfolio, multiple asset classes.
Feroz Siddiqui: Yeah. And all listed here instead of these holdings.

00:03:27
Sami Sayyed: Perfect. Perfect. Understood. All listed.
Feroz Siddiqui: When I click stocks then it should show me these
Sami Sayyed: Mhm.
Feroz Siddiqui: holdings.
Sami Sayyed: Perfect. Understood. When you
Feroz Siddiqui: When I click suk it should show me all my suks and when I click sukooks and there is
Sami Sayyed: drop,
Feroz Siddiqui: nothing then there should be a plus button saying that add
Sami Sayyed: okay,
Feroz Siddiqui: sukook.
Sami Sayyed: perfect. I got completely got it.
Feroz Siddiqui: Okay,
Sami Sayyed: Okay.
Feroz Siddiqui: at that time you can see if if if you have an API API for it, it should prompt saying that would you want to connect to this uh you know whatever IBKR or
Sami Sayyed: Mhm.
Feroz Siddiqui: whatever you know then you let them connect directly.
Sami Sayyed: to get the live. Okay. Yeah, that's that's perfect. In the graph section,
Feroz Siddiqui: Yeah.
Sami Sayyed: um you want multiple graphs as you said, right?
Feroz Siddiqui: Yeah.
Sami Sayyed: Like when we add multiple asset classes.
Feroz Siddiqui: So for every asset class there should be a different colored

00:04:11
Sami Sayyed: Okay.
Feroz Siddiqui: graph but the problem is that
Sami Sayyed: Different colored graph.
Feroz Siddiqui: scale like this one is showing 250,000 bond is uh one bond is 200,000 if I have five bonds that's a million dollars how will that show in the same
Sami Sayyed: Yeah. So we can do one thing.
Feroz Siddiqui: graph yeah no the whole
Sami Sayyed: We can add like a drop down over here and then they can select which they want to see in the craft.
Feroz Siddiqui: the whole idea is that dashboard should collect everything together and show
Sami Sayyed: Okay. But this is the dashboard, right? This is this is for your individual portfolio.
Feroz Siddiqui: Yes,
Sami Sayyed: This is the main d.
Feroz Siddiqui: one one portfolio. There's only one asset class here. Now imagine if there are three asset classes,
Sami Sayyed: Okay,
Feroz Siddiqui: ETFs, stocks, and suk. What what will be will it look like? You I don't mind if you add all of that together and show this graph.
Sami Sayyed: cool.
Feroz Siddiqui: Instead of 247,000, it'll show 1,ion247,000.

00:05:01
Sami Sayyed: So, we can do that,
Feroz Siddiqui: You get it?
Sami Sayyed: right? Like if it shows all together, it's
Feroz Siddiqui: Oh yeah.
Sami Sayyed: fine.
Feroz Siddiqui: at the at this level at the dashboard level it can all be together but when I click suk it should show me a suk graph only when I click
Sami Sayyed: Ah, perfect. Perfect. So, when you select Here you can see
Feroz Siddiqui: only total yeah because that excel sheet
Sami Sayyed: amazing. Absolutely.
Feroz Siddiqui: spreadsheet so that's how it and the main thing is
Sami Sayyed: So uh Mhm. So from a UI
Feroz Siddiqui: adding an asset class should be easy so moment I go to the portfolio I see all the 10 asset classes I click
Sami Sayyed: perspective
Feroz Siddiqui: on stocks if I don't have any holdings it should show me a popup saying that add new account you know and then da add new whatever mumu or ibkr or whatever or csv or whatever all that if I already have it then it should show me like
Sami Sayyed: Perfect.

00:05:47
Feroz Siddiqui: this then if I click on suks it'll say oh you don't have any
Sami Sayyed: Understood.
Feroz Siddiqui: suks please add now
Sami Sayyed: Uh so it should it should be you you should be able to add from this itself easier
Feroz Siddiqui: yes it should be visible not having to go to settings and
Sami Sayyed: process. Mhm.
Feroz Siddiqui: create
Sami Sayyed: Also my plan is you know before the soft launch we'll create like an onboarding flow. So if someone you know downloads the app the app will show them see this is the dash there will be like an arrow you know you see sometimes you show them this is
Feroz Siddiqui: yeah like the banks like like the banks like access bank every time the app uh updates it does
Sami Sayyed: yeah exactly the same thing same thing you can do
Feroz Siddiqui: that okay bank
Sami Sayyed: that okay so this page is done now next one is this page insights page
Feroz Siddiqui: account can I should be able to put multiple bank accounts. So bank account is an asset class.
Sami Sayyed: okay bank account is an asset class Perfect.

00:06:39
Feroz Siddiqui: Yeah. And the holdings are the bank accounts themselves. Access bank, ICICI bank,
Sami Sayyed: Mhm.
Feroz Siddiqui: DBS bank, you know, like that. So that that's the way it should be. So you so you can actually add different bank accounts and every bank account you should be able to change the currency.
Sami Sayyed: Okay. Change the
Feroz Siddiqui: Yeah.
Sami Sayyed: currency.
Feroz Siddiqui: Uh allow users to enter the currency. But the dashboard should be in US dollars.
Sami Sayyed: Mhm.
Feroz Siddiqui: And that also they can change. They can choose to say that I want the dashboard in PKR, INR, whatever. They should be able to change it in the settings,
Sami Sayyed: Understood.
Feroz Siddiqui: master currency or whatever, primary currency or something like that.
Sami Sayyed: Perfect.
Feroz Siddiqui: You should name it. And then for banks, you can put so if I have a bank account in Saudi Arabia, it is already S.
Sami Sayyed: Amazing. Got it. Understood.
Feroz Siddiqui: So in in the bank account you should always mention country but

00:07:32
Sami Sayyed: Mhm. So over here you can see there two tabs um investments and okay here
Feroz Siddiqui: remember remember wait a bank account do
Sami Sayyed: you
Feroz Siddiqui: not map the country with the currency because I can have a account in Singapore but I can have US dollars in it unlike India.
Sami Sayyed: Okay.
Feroz Siddiqui: Yeah it international look. So I have account in uh Saudi Arabia.
Sami Sayyed: Mhm.
Feroz Siddiqui: I have S and uh US dollar. The my Dubai account has AED and USD.
Sami Sayyed: Perfect. I got
Feroz Siddiqui: Yeah. And Singapore I've got seven currencies.
Sami Sayyed: it.
Feroz Siddiqui: It's a multicurrency account. So let users choose the amount that they are putting into the bank account.
Sami Sayyed: Yes. Amount and currency. Okay.
Feroz Siddiqui: Yeah. So if users currencies like in DBS I have both US dollars and uh this thing either I can just
Sami Sayyed: Segregate.
Feroz Siddiqui: consolidate and put put it down as one USD file USD amount or I can create like another bank uh in the same asset class I created as another holding.

00:08:34
Sami Sayyed: Perfect.
Feroz Siddiqui: So one DBS is US dollar.
Sami Sayyed: Okay.
Feroz Siddiqui: The other DBS is SGD or whatever I
Sami Sayyed: SGD. Perfect.
Feroz Siddiqui: want.
Sami Sayyed: Mhm. I understand. So this is the net.
Feroz Siddiqui: And like the multiple bank accounts I should be able to put
Sami Sayyed: Uh-huh.
Feroz Siddiqui: it.
Sami Sayyed: Multiple bank accounts. Okay.
Feroz Siddiqui: Bank accounts are holdings.
Sami Sayyed: Perfect.
Feroz Siddiqui: So bank accounts are uh asset classes but each individual bank is a
Sami Sayyed: Mhm.
Feroz Siddiqui: holding.
Sami Sayyed: Okay.
Feroz Siddiqui: Got
Sami Sayyed: Perfect. Perfect. Understood.
Feroz Siddiqui: it.
Sami Sayyed: Individual bank is a holding and multip and bank account is the asset class. Perfect.
Feroz Siddiqui: Yeah. Yeah.
Sami Sayyed: Mhm.
Feroz Siddiqui: 10
Sami Sayyed: Yes. Yes. 10 minutes. So yeah, this is the second thing.
Feroz Siddiqui: minutes.
Sami Sayyed: Um this is the network. You can see this is the dashboard and we can see there are two options.

00:09:24
Sami Sayyed: There's the net worth option also. So this shows you if you have multiple assets here like see add asset. Okay, there's an option add asset. Okay. But these are this is for this one. property, vehicle, collectible, other asset. You can add your bank
Feroz Siddiqui: Vehicle vehicle I don't think you should put because it skews the whole thing you know I might have a Rolls-Royce but that's like
Sami Sayyed: account
Feroz Siddiqui: a million real and then it it's a depreciating asset you see so don't put any depreciating asset only put financial assets yeah property
Sami Sayyed: only financial assets. Perfect. Collect collectibles.
Feroz Siddiqui: is collectibles is fine watches and
Sami Sayyed: Okay,
Feroz Siddiqui: things like that. Yeah. Precious.
Sami Sayyed: I'll remove vehicles.
Feroz Siddiqui: Yeah. Remove vehicle.
Sami Sayyed: Remove vehicles. So, this is this shows you your full net worth. Okay, like once you add your bank accounts, all the assets, um it shows you your net worth over here in this

00:10:14
Feroz Siddiqui: Yeah. In the property I should be able to mention that this property is rented out and this is the
Sami Sayyed: section.
Feroz Siddiqui: monthly rental or annual rental. So add add property.
Sami Sayyed: Not Okay, one second. Add property. Continue. Create and link a mortgage. Okay. So, let me just put a name US
Feroz Siddiqui: No. So here you should say rented rental and start date and end
Sami Sayyed: option should be there. Rental or okay.
Feroz Siddiqui: date for rental.
Sami Sayyed: Start date and end date for rental. start date and I I can just imagine you know how powerful the app will be you know once we have all these things no
Feroz Siddiqui: That's why I'm so excited. The end date should be optional.
Sami Sayyed: one okay perfect
Feroz Siddiqui: If they don't put the end date,
Sami Sayyed: start okay perfect okay perfect
Feroz Siddiqui: it should should allow you to go to the next uh thing. That means it's perpetual. Perpetual means for

00:11:15
Sami Sayyed: okay started perfect I got that I got that
Feroz Siddiqui: life.
Sami Sayyed: noted
Feroz Siddiqui: And that monthly should add to the monthly uh assets uh you know collection uh it should go to the bank or whatever. What are you doing with the rental income? It should add to your net
Sami Sayyed: add to your net worth.
Feroz Siddiqui: worth.
Sami Sayyed: Okay. Net worth. Okay. Every month it should update,
Feroz Siddiqui: Yeah. But there's a problem.
Sami Sayyed: right?
Feroz Siddiqui: If if the bank account is linked automatically, then you're you're updating it here and it's also updated in the cash flow in the bank and that's
Sami Sayyed: No,
Feroz Siddiqui: automatically
Sami Sayyed: I don't think we'll be able to link it. I just let me check if we can do that.
Feroz Siddiqui: being
Sami Sayyed: Or do you think it would be better if it's normal? Just manually we add the price there. I mean the
Feroz Siddiqui: Yeah. Yeah. Yeah. Sure. That's fine.

00:12:00
Sami Sayyed: balance
Feroz Siddiqui: For now, we can do that. Yeah. Don't don't have to make it perfect at this stage.
Sami Sayyed: we can do.
Feroz Siddiqui: Okay.
Sami Sayyed: Okay, that's that's perfect. One last Okay, one last thing. Okay. Um this one in the goal tracking um you can see there are multiple things we can track. So do you need any addition here? We have retirement, education, home purchase, savings, weddings, and we should have option for custom goal also, right?
Feroz Siddiqui: Yes.
Sami Sayyed: Savings goal, something I'm saving for. So, I'll create this goal. What do you think about this dashboard over here? I can add my portfolios here. So, it links to this goal and should be able to show you. So,
Feroz Siddiqui: But what is the purpose of this? Does it prompt the person for it or
Sami Sayyed: Yeah, it should show you. Just one second.
Feroz Siddiqui: what?
Sami Sayyed: Add it here,
Feroz Siddiqui: Okay. add add liabilities as well because because this is a net

00:13:01
Sami Sayyed: liabilities.
Feroz Siddiqui: worth right.
Sami Sayyed: Mhm.
Feroz Siddiqui: So in the asset class you should
Sami Sayyed: Mhm. Just one second. Go in there.
Feroz Siddiqui: also
Sami Sayyed: Go here. net worth and we go here. So add liabilities there.
Feroz Siddiqui: is there
Sami Sayyed: Yeah.
Feroz Siddiqui: okay?
Sami Sayyed: So it's like a drop down auto loan, credit card loan.
Feroz Siddiqui: Yeah. Yeah. Okay. So, you can add as many as you want, right? So, the most important thing,
Sami Sayyed: Yes,
Feroz Siddiqui: what does it do with this? Does it reduce that in the net worth or what does it
Sami Sayyed: it does. It does reduce it. See, add a liability to track against your networks.
Feroz Siddiqui: do?
Sami Sayyed: It will have a life factor once you set the current balance and the data. Mhm.
Feroz Siddiqui: Otherwise, it'll be there for life.
Sami Sayyed: Mhm.
Feroz Siddiqui: balance balance
Sami Sayyed: Origination date.
Feroz Siddiqui: date.
Sami Sayyed: Okay. Yeah.

00:14:04
Sami Sayyed: This is for adding the balance used to track depth.
Feroz Siddiqui: No current what is current balance liability current okay balance date
Sami Sayyed: So
Feroz Siddiqui: origination date meaning start date right
Sami Sayyed: uh it should yeah it should the start date okay balance that I'll
Feroz Siddiqui: basically you should have start date and end date balance date so have you should have a start date of the
Sami Sayyed: remove start date
Feroz Siddiqui: loan so like for example somebody's already got a property right and he's been paying it for the last five years so he'll go and put
Sami Sayyed: yes
Feroz Siddiqui: the five year old date here he'll 2021. Okay.
Sami Sayyed: ready Mhm.
Feroz Siddiqui: And current date system and end date. So that that shows that how many year loan is it or instead of putting end date you say uh
Sami Sayyed: Yes.
Feroz Siddiqui: um uh loan loan how many years loan is it percentage as
Sami Sayyed: Okay.
Feroz Siddiqui: well if you want.
Sami Sayyed: Mhm. Understood. Personally, how many years?
Feroz Siddiqui: Yeah.
Sami Sayyed: Amazing.

00:15:03
Sami Sayyed: Amazing.
Feroz Siddiqui: Then it'll automatically calculate everything.
Sami Sayyed: Okay. automatically calculates everything.
Feroz Siddiqui: your then you put there one something called EMI
Sami Sayyed: Okay. Am I should be here? No, it's not here.
Feroz Siddiqui: no it can't be there EMI is not a liability EMI is your uh you know
Sami Sayyed: BMI.
Feroz Siddiqui: monthly what you're
Sami Sayyed: What you're paying installments. Okay.
Feroz Siddiqui: paying
Sami Sayyed: Perfect. Okay. Yeah. That's that's all you know. This is the investments assets and liabilities.
Feroz Siddiqui: Okay.
Sami Sayyed: You can add assets
Feroz Siddiqui: Okay. So, next time when we when we talk on Sunday,
Sami Sayyed: here.
Feroz Siddiqui: you I want to see some uh uh all of these asset classes and all of that dummy, you know,
Sami Sayyed: Yes.
Feroz Siddiqui: like put put in something. Meanwhile,
Sami Sayyed: Mhm.
Feroz Siddiqui: if I complete my Excel sheet, I will I will share it with
Sami Sayyed: Yes. 100%. I'll do it. I'll do it.
Feroz Siddiqui: you.
Sami Sayyed: I'll do all of this now. I got the direction. I was working on some bug fixes in the app. Yeah. I'll add the features now.
Feroz Siddiqui: Okay.
Sami Sayyed: Yes.
Feroz Siddiqui: Okay. I've written it down. Thank you so much.
Sami Sayyed: Thank you so much.
Feroz Siddiqui: Take care.
Sami Sayyed: Thank you so much.
Feroz Siddiqui: God bless. Good job.
Sami Sayyed: Allah first.

Transcription ended after 00:16:28
```
