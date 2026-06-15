import { createContext } from 'react';
import type { BrandingSettings, PublicSiteSettings } from '@/types/api';

export interface SiteSettingsContextValue {
  settings: PublicSiteSettings | null;
  branding: BrandingSettings;
  refreshSiteSettings: () => Promise<void>;
}

export const SiteSettingsContext = createContext<SiteSettingsContextValue | undefined>(undefined);
