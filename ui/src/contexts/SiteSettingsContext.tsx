import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { apiService } from '@/services/api';
import type { BrandingSettings, PublicSiteSettings } from '@/types/api';
import { SiteSettingsContext } from './site-settings-context';

const DEFAULT_BRANDING: BrandingSettings = {
  site_title: 'OxideDB',
  site_description: 'A hook-first database with plugin architecture',
  logo_url: undefined,
  favicon_url: undefined,
  primary_color: '#0f7490',
  secondary_color: '#64748b',
  custom_css: undefined,
  footer_text: 'Powered by OxideDB',
};

export function SiteSettingsProvider({ children }: { children: React.ReactNode }) {
  const [settings, setSettings] = useState<PublicSiteSettings | null>(null);

  const refreshSiteSettings = useCallback(async () => {
    try {
      const nextSettings = await apiService.getPublicSiteSettings();
      setSettings(nextSettings);
    } catch (error) {
      console.warn('Failed to load public site settings:', error);
    }
  }, []);

  useEffect(() => {
    void refreshSiteSettings();
  }, [refreshSiteSettings]);

  const branding = useMemo(
    () => ({ ...DEFAULT_BRANDING, ...settings?.branding }),
    [settings]
  );

  useEffect(() => {
    applyBranding(branding);
  }, [branding]);

  const value = useMemo(
    () => ({
      settings,
      branding,
      refreshSiteSettings,
    }),
    [branding, refreshSiteSettings, settings]
  );

  return (
    <SiteSettingsContext.Provider value={value}>
      {children}
    </SiteSettingsContext.Provider>
  );
}

function applyBranding(branding: BrandingSettings) {
  const title = branding.site_title?.trim() || DEFAULT_BRANDING.site_title;
  document.title = `${title} Admin`;

  if (branding.favicon_url) {
    upsertFavicon(branding.favicon_url);
  }

  setThemeColor('--primary', branding.primary_color);
  setThemeColor('--ring', branding.primary_color);
  setThemeColor('--sidebar-primary', branding.primary_color);
  setThemeColor('--secondary', branding.secondary_color);
  upsertCustomCss(branding.custom_css);
}

function setThemeColor(variable: string, value?: string) {
  const hsl = hexToHsl(value);
  if (!hsl) {
    return;
  }

  document.documentElement.style.setProperty(variable, hsl);
}

function upsertFavicon(href: string) {
  let icon = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
  if (!icon) {
    icon = document.createElement('link');
    icon.rel = 'icon';
    document.head.appendChild(icon);
  }

  icon.href = href;
}

function upsertCustomCss(css?: string) {
  const id = 'oxidedb-site-custom-css';
  let style = document.getElementById(id);

  if (!css?.trim()) {
    style?.remove();
    return;
  }

  if (!style) {
    style = document.createElement('style');
    style.id = id;
    document.head.appendChild(style);
  }

  style.textContent = css;
}

function hexToHsl(value?: string): string | null {
  if (!value || !/^#[0-9a-fA-F]{6}$/.test(value)) {
    return null;
  }

  const red = parseInt(value.slice(1, 3), 16) / 255;
  const green = parseInt(value.slice(3, 5), 16) / 255;
  const blue = parseInt(value.slice(5, 7), 16) / 255;

  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  const lightness = (max + min) / 2;
  const delta = max - min;

  if (delta === 0) {
    return `0 0% ${Math.round(lightness * 100)}%`;
  }

  const saturation = delta / (1 - Math.abs(2 * lightness - 1));
  const hue =
    max === red
      ? 60 * (((green - blue) / delta) % 6)
      : max === green
        ? 60 * ((blue - red) / delta + 2)
        : 60 * ((red - green) / delta + 4);

  const normalizedHue = hue < 0 ? hue + 360 : hue;

  return `${Math.round(normalizedHue)} ${Math.round(saturation * 100)}% ${Math.round(lightness * 100)}%`;
}
