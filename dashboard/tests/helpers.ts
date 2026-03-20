import type { Page } from '@playwright/test';

/** Plugin name as registered in the SDK */
export const PLUGIN_NAME = 'plugin-coding-pack';

/** Base route for SDK-routed plugin pages */
export const PLUGIN_BASE = `/plugins/${PLUGIN_NAME}`;

/**
 * Initialize page: navigate home, wait for app shell, handle API key form.
 */
export async function initPage(page: Page): Promise<void> {
  await page.goto('/', { waitUntil: 'load', timeout: 15000 });

  // Wait for the app shell (sidebar or main) to actually render
  await page.waitForSelector('aside, main, .app-layout', { timeout: 10000 });
  await page.waitForTimeout(500);

  // If API key form is shown, skip it
  const skip = page.locator('button:has-text("Skip")');
  if (await skip.isVisible({ timeout: 1500 }).catch(() => false)) {
    await skip.click();
    await page.waitForTimeout(500);
  }
}

/**
 * Navigate to a page and wait for content to render.
 */
export async function goTo(page: Page, path: string): Promise<void> {
  await page.goto(path, { waitUntil: 'load', timeout: 15000 });
  await page.waitForSelector('aside, main, .app-layout', { timeout: 10000 });
  await page.waitForTimeout(500);
}

/**
 * Navigate to a plugin page via SDK route.
 * @param page Playwright page
 * @param subpath Plugin subpath (e.g. "/overview", "/workflows")
 */
export async function goToPlugin(page: Page, subpath: string): Promise<void> {
  await goTo(page, `${PLUGIN_BASE}${subpath}`);
}
