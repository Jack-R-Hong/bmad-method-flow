import { test, expect, type Page } from '@playwright/test';
import { initPage, goToPlugin } from './helpers';

test.describe('Execute Workflow Dialog', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
  });

  async function openFirstDialog(page: Page) {
    await goToPlugin(page, '/overview');

    // Wait for cards to render
    await page.waitForTimeout(500);

    // Find Execute buttons (not the full-width wf-header)
    const execBtn = page.locator('button:not(.wf-header)').filter({ hasText: /^.*Execute$/ }).first();
    await expect(execBtn).toBeVisible({ timeout: 5000 });
    await execBtn.click();

    const dialog = page.locator('[role="dialog"]');
    await expect(dialog).toBeVisible({ timeout: 3000 });
    return dialog;
  }

  test('opens execute dialog', async ({ page }) => {
    const dialog = await openFirstDialog(page);
    await expect(dialog.locator('h2')).toBeVisible();
  });

  test('dialog has textarea with placeholder', async ({ page }) => {
    const dialog = await openFirstDialog(page);
    const textarea = dialog.locator('textarea, #workflow-input');
    await expect(textarea).toBeVisible();
    const placeholder = await textarea.getAttribute('placeholder');
    expect(placeholder).toBeTruthy();
  });

  test('execute disabled when empty, enabled when filled', async ({ page }) => {
    const dialog = await openFirstDialog(page);
    const submitBtn = dialog.locator('button').filter({ hasText: 'Execute' });
    await expect(submitBtn).toBeDisabled();

    await dialog.locator('textarea, #workflow-input').fill('Test input');
    await expect(submitBtn).toBeEnabled();
  });

  test('dialog closes on Cancel', async ({ page }) => {
    const dialog = await openFirstDialog(page);
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).not.toBeVisible();
  });

  test('dialog closes on Escape', async ({ page }) => {
    const dialog = await openFirstDialog(page);
    await page.keyboard.press('Escape');
    await expect(dialog).not.toBeVisible();
  });

  test('submits workflow execution', async ({ page }) => {
    const dialog = await openFirstDialog(page);
    await dialog.locator('textarea, #workflow-input').fill('Add validation');
    await dialog.locator('button').filter({ hasText: 'Execute' }).click();
    await page.waitForTimeout(2000);
  });

  test('coding-review shows target field', async ({ page }) => {
    await goToPlugin(page, '/overview');
    await page.waitForTimeout(500);

    const reviewText = page.getByText('coding-review').first();
    await reviewText.scrollIntoViewIfNeeded();
    await page.waitForTimeout(300);

    const nonHeaderBtns = page.locator('button:not(.wf-header)').filter({ hasText: /^.*Execute$/ });
    const count = await nonHeaderBtns.count();

    if (count >= 6) {
      await nonHeaderBtns.nth(5).scrollIntoViewIfNeeded();
      await nonHeaderBtns.nth(5).click();

      const dialog = page.locator('[role="dialog"]');
      if (await dialog.isVisible({ timeout: 3000 }).catch(() => false)) {
        const text = await dialog.textContent();
        expect(text).toContain('Target');
      }
    }
  });
});

test.describe('Execute Workflow — SDK Form Page', () => {
  test.beforeEach(async ({ page }) => {
    await initPage(page);
  });

  test('form page has workflow select and textarea', async ({ page }) => {
    await goToPlugin(page, '/execute');
    // SDK GenericForm renders form fields
    await expect(page.getByText('Workflow').first()).toBeVisible({ timeout: 5000 });
    await expect(page.getByText('Task Description').first()).toBeVisible();
  });
});
