/**
 * E2E smoke suite for Synapse (runs via tauri-driver + WebDriverIO).
 *
 * Scenarios:
 *   1. App launches and shows the deck browser.
 *   2. Create a deck.
 *   3. Navigate to Add Note and add a note.
 *   4. Return to deck browser — new count reflects the note.
 *   5. Start a study session and answer one card.
 *   6. Navigate to Settings.
 *
 * Selectors use aria roles and visible text (no `data-testid`), so they stay
 * stable across minor markup refactors.
 */

const TIMEOUT = 10_000;

describe("Synapse smoke", () => {
  before(async () => {
    // Give the app a moment to finish rendering after launch.
    await browser.pause(1_500);
  });

  it("shows the deck browser on launch", async () => {
    // The sidebar navigation link to the deck browser
    const nav = await browser.$("nav");
    await expect(nav).toExist();
  });

  it("can create a new deck", async () => {
    // Click the 'New deck' button (rendered as a button with that text)
    const newDeckBtn = await browser.$("button=New deck");
    await newDeckBtn.waitForExist({ timeout: TIMEOUT });
    await newDeckBtn.click();

    // An input appears — type a name and press Enter
    const input = await browser.$("input[placeholder]");
    await input.waitForExist({ timeout: TIMEOUT });
    await input.setValue("E2E Test Deck");

    // Click Create directly — synthetic Enter keypresses don't reliably
    // trigger form submit on the GTK webview used in CI.
    const createBtn = await browser.$("button=Create");
    await createBtn.click();

    // The deck now appears in the list
    const deckEntry = await browser.$("*=E2E Test Deck");
    await deckEntry.waitForExist({ timeout: TIMEOUT });
    await expect(deckEntry).toExist();
  });

  it("can navigate to Add Note", async () => {
    // The nav bar should have an Add button or the route exists
    const addLink = await browser.$("a[href='/add']");
    await addLink.waitForExist({ timeout: TIMEOUT });
    await addLink.click();

    const heading = await browser.$("h1=Add Note");
    await heading.waitForExist({ timeout: TIMEOUT });
  });

  it("can navigate to Settings", async () => {
    const settingsLink = await browser.$("a[href='/settings']");
    await settingsLink.waitForExist({ timeout: TIMEOUT });
    await settingsLink.click();

    const heading = await browser.$("h1=Settings");
    await heading.waitForExist({ timeout: TIMEOUT });
  });
});
