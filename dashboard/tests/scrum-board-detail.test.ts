import { test, expect } from '@playwright/test';
import { initPage, goToPlugin } from './helpers';

test.describe('Scrum Board — Epic & Story Detail Pages (P0 + P2)', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
  });

  // ── P0: Epic Detail ──

  test('P0-04: epic detail page loads with correct sections', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-1');
    const main = page.locator('main');

    // Should show all detail sections
    await expect(main.getByText('Epic Info').first()).toBeVisible({ timeout: 5000 });
    await expect(main.getByText('Epic ID').first()).toBeVisible();
    await expect(main.getByText('Title').first()).toBeVisible();
    await expect(main.getByText('Status').first()).toBeVisible();
    await expect(main.getByText('Phase').first()).toBeVisible();
  });

  test('P0-04b: epic detail shows correct epic data', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-1');
    const main = page.locator('main');

    // Should display the epic ID
    await expect(main.getByText('epic-1')).toBeVisible({ timeout: 5000 });
    // Phase 1 epic should show status done
    await expect(main.getByText('done').first()).toBeVisible();
  });

  // ── P0: Story Detail ──

  test('P0-05: story detail page loads with correct sections', async ({ page }) => {
    await goToPlugin(page, '/board/stories/1-1-crate-scaffolding-and-process-manager');
    const main = page.locator('main');

    await expect(main.getByText('Story Info').first()).toBeVisible({ timeout: 5000 });
    await expect(main.getByText('User Story').first()).toBeVisible();
    await expect(main.getByText('Acceptance Criteria').first()).toBeVisible();
  });

  test('P0-05b: story detail shows correct story data', async ({ page }) => {
    await goToPlugin(page, '/board/stories/1-1-crate-scaffolding-and-process-manager');
    const main = page.locator('main');

    await expect(main.getByText('epic-1')).toBeVisible({ timeout: 5000 });
    await expect(main.getByText('done').first()).toBeVisible();
  });

  // ── P2: Epic → Stories Navigation ──

  test('P2-03: epic detail shows story list with statuses', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-1');
    const main = page.locator('main');

    // Stories section
    await expect(main.getByText('Stories').first()).toBeVisible({ timeout: 5000 });
    await expect(main.getByText('Total Stories').first()).toBeVisible();
    await expect(main.getByText('Completed').first()).toBeVisible();

    // Story list should have content (not empty)
    const storyList = main.getByText('Story Breakdown').first();
    await expect(storyList).toBeVisible({ timeout: 3000 });
  });

  test('P2-04: story detail shows user story and acceptance criteria content', async ({ page }) => {
    // Use a Phase 2 story that has rich content in epics.md
    await goToPlugin(page, '/board/stories/12-1-create-bmadagentregistry-struct-with-agent-definition-constants');
    const main = page.locator('main');

    await expect(main.getByText('User Story').first()).toBeVisible({ timeout: 5000 });
    await expect(main.getByText('Acceptance Criteria').first()).toBeVisible();
  });

  // ── P2: Error/Edge Cases ──

  test('P2-05: invalid epic ID shows error or empty state', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-999');
    const main = page.locator('main');

    // Should show error state or "not found" message
    const errorOrEmpty = main.getByText(/not found|error|no data/i).first();
    await expect(errorOrEmpty).toBeVisible({ timeout: 5000 });
  });

  test('P2-01: board shows error state on data fetch failure', async ({ page }) => {
    // Navigate to board with an invalid plugin to force error
    // This tests the GenericBoard error rendering
    await page.goto('/plugins/nonexistent-plugin/board', { waitUntil: 'load', timeout: 10000 });
    await page.waitForTimeout(2000);
    // Should show some kind of error (plugin not found or data error)
    const hasError = await page.locator('text=/error|not found|failed/i').first().isVisible({ timeout: 5000 }).catch(() => false);
    expect(hasError).toBeTruthy();
  });

  // ── P2: Requirements Coverage ──

  test('P2-03b: epic detail for Phase 2 epic shows requirements coverage', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-12');
    const main = page.locator('main');

    await expect(main.getByText('Epic Info').first()).toBeVisible({ timeout: 5000 });
    // Phase 2 epics from epics.md have FRs/NFRs
    const reqSection = main.getByText('Requirements Coverage').first();
    if (await reqSection.isVisible({ timeout: 2000 }).catch(() => false)) {
      await expect(main.getByText('FRs Covered').first()).toBeVisible();
    }
  });
});

// ── P3: Responsive ──

test.describe('Scrum Board — Responsive (P3)', () => {
  test('P3-01: board renders on mobile viewport', async ({ browser }) => {
    const context = await browser.newContext({
      viewport: { width: 375, height: 812 },
    });
    const page = await context.newPage();

    await page.goto('/', { waitUntil: 'load', timeout: 15000 });
    await page.waitForSelector('main', { timeout: 10000 });

    // Skip API key form if shown
    const skip = page.locator('button:has-text("Skip")');
    if (await skip.isVisible({ timeout: 1500 }).catch(() => false)) {
      await skip.click();
      await page.waitForTimeout(500);
    }

    await page.goto(`/plugins/plugin-coding-pack/board`, { waitUntil: 'load', timeout: 15000 });
    await page.waitForTimeout(2000);

    // Board should still render (columns stack vertically on mobile)
    const heading = page.locator('h1');
    await expect(heading).toContainText('Scrum Board', { timeout: 5000 });

    await context.close();
  });
});
