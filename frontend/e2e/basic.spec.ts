import { test, expect } from '@playwright/test';

test('homepage loads', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveTitle(/Universus/);
});

test('galaxy page loads', async ({ page }) => {
  await page.goto('/galaxy.html');
  await expect(page.locator('#galaxyCanvas')).toBeVisible();
});

test('leaderboard loads', async ({ page }) => {
  await page.goto('/leaderboard.html');
  await expect(page.locator('.leaderboard-table')).toBeVisible();
});

test('shop page loads', async ({ page }) => {
  await page.goto('/shop.html');
  await expect(page.locator('.shop-container')).toBeVisible();
});