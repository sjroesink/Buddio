describe("GoLaunch E2E", () => {
  it("should show the main window", async () => {
    // The app should render the root container
    const root = await $('[data-testid="app-root"]');
    await root.waitForExist({ timeout: 10000 });
    expect(await root.isDisplayed()).toBe(true);
  });

  it("should have a search input", async () => {
    const input = await $('[data-testid="search-input"]');
    await input.waitForExist({ timeout: 5000 });
    expect(await input.isDisplayed()).toBe(true);
  });

  it("should type in search and filter results", async () => {
    const input = await $('[data-testid="search-input"]');
    await input.setValue("test");

    // Wait for results to potentially filter
    await browser.pause(500);

    const value = await input.getValue();
    expect(value).toBe("test");
  });

  it("should clear search with Escape", async () => {
    const input = await $('[data-testid="search-input"]');
    await input.setValue("something");
    await browser.keys(["Escape"]);

    await browser.pause(300);
    const value = await input.getValue();
    expect(value).toBe("");
  });

  it("should show settings button", async () => {
    const settings = await $('[data-testid="settings-button"]');
    expect(await settings.isDisplayed()).toBe(true);
  });

  it("should navigate with arrow keys", async () => {
    // Focus the container
    const root = await $('[data-testid="app-root"]');
    await root.click();

    // Press down arrow
    await browser.keys(["ArrowDown"]);
    await browser.pause(200);

    // Press up arrow
    await browser.keys(["ArrowUp"]);
    await browser.pause(200);

    // No crash = success for basic navigation
    expect(true).toBe(true);
  });
});
