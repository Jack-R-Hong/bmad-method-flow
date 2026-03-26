import { test, expect } from '@playwright/test';
import { initPage, goToPlugin, PLUGIN_BASE } from './helpers';

test.describe('Scrum Board — Board, Epic Detail, Story Detail', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
  });

  // ── Board Page ──

  test('navigates to scrum board from sidebar', async ({ page }) => {
    const link = page.locator(`aside a[href="${PLUGIN_BASE}/board"]`);
    await expect(link).toBeVisible();
    await link.click();
    await page.waitForURL(`**${PLUGIN_BASE}/board`);
    await expect(page.locator('h1')).toContainText('Scrum Board');
  });

  test('board page renders 5 status columns', async ({ page }) => {
    await goToPlugin(page, '/board');
    const main = page.locator('main');
    await expect(main.getByText('Backlog')).toBeVisible({ timeout: 5000 });
    await expect(main.getByText('Ready')).toBeVisible();
    await expect(main.getByText('In Progress')).toBeVisible();
    await expect(main.getByText('Review')).toBeVisible();
    await expect(main.getByText('Done')).toBeVisible();
  });

  test('board page shows summary stats', async ({ page }) => {
    await goToPlugin(page, '/board');
    const main = page.locator('main');
    // Summary should show progress percentage
    await expect(main.locator('text=/\\d+\\.\\d+%/')).toBeVisible({ timeout: 5000 });
  });

  test('board page displays epic cards', async ({ page }) => {
    await goToPlugin(page, '/board');
    const main = page.locator('main');
    // Should show at least one epic
    await expect(main.locator('[data-type="epic"]').first()).toBeVisible({ timeout: 5000 });
  });

  test('board page displays story cards', async ({ page }) => {
    await goToPlugin(page, '/board');
    const main = page.locator('main');
    // Should show at least one story
    await expect(main.locator('[data-type="story"]').first()).toBeVisible({ timeout: 5000 });
  });

  test('board page has filter controls', async ({ page }) => {
    await goToPlugin(page, '/board');
    const main = page.locator('main');
    // Phase filter
    await expect(main.locator('select, [role="combobox"]').first()).toBeVisible({ timeout: 5000 });
  });

  // ── Epic Detail Page ──

  test('epic detail page shows epic info', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-1');
    const main = page.locator('main');
    await expect(main.locator('text=Epic Info')).toBeVisible({ timeout: 5000 });
    await expect(main.locator('text=epic-1')).toBeVisible();
    await expect(main.locator('text=done')).toBeVisible();
  });

  test('epic detail page shows stories section', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-1');
    const main = page.locator('main');
    await expect(main.locator('text=Stories')).toBeVisible({ timeout: 5000 });
    await expect(main.locator('text=Total Stories')).toBeVisible();
    await expect(main.locator('text=Completed')).toBeVisible();
  });

  test('epic detail page shows requirements coverage', async ({ page }) => {
    await goToPlugin(page, '/board/epics/epic-12');
    const main = page.locator('main');
    await expect(main.locator('text=Requirements Coverage')).toBeVisible({ timeout: 5000 });
  });

  // ── Story Detail Page ──

  test('story detail page shows story info', async ({ page }) => {
    await goToPlugin(page, '/board/stories/1-1-crate-scaffolding-and-process-manager');
    const main = page.locator('main');
    await expect(main.locator('text=Story Info')).toBeVisible({ timeout: 5000 });
    await expect(main.locator('text=epic-1')).toBeVisible();
    await expect(main.locator('text=done')).toBeVisible();
  });

  test('story detail page shows acceptance criteria section', async ({ page }) => {
    await goToPlugin(page, '/board/stories/12-1-create-bmadagentregistry-struct-with-agent-definition-constants');
    const main = page.locator('main');
    await expect(main.locator('text=Acceptance Criteria')).toBeVisible({ timeout: 5000 });
  });

  test('story detail page shows user story section', async ({ page }) => {
    await goToPlugin(page, '/board/stories/12-1-create-bmadagentregistry-struct-with-agent-definition-constants');
    const main = page.locator('main');
    await expect(main.locator('text=User Story')).toBeVisible({ timeout: 5000 });
  });
});
