import { useEffect, useMemo, useState } from "react";
import { Puzzle, RefreshCw } from "lucide-react";
import { useParams } from "react-router-dom";

import PageLayout from "@/components/PageLayout";
import { Button } from "@/components/ui/button";
import { ErrorState, LoadingState } from "@/components/admin/AdminState";
import { useTheme } from "@/components/use-theme";
import { apiService } from "@/services/api";
import type { PluginAdminPage as PluginAdminPageInfo } from "@/types/api";

const PluginAdminPage = () => {
  const { pluginName, pageSlug } = useParams();
  const { theme } = useTheme();
  const [pages, setPages] = useState<PluginAdminPageInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchPages = async () => {
    try {
      setLoading(true);
      setError(null);
      setPages(await apiService.getPluginAdminPages());
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load plugin page");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchPages();
  }, []);

  const page = useMemo(
    () =>
      pages.find(
        (candidate) =>
          candidate.plugin_name === pluginName && candidate.slug === pageSlug
      ),
    [pageSlug, pages, pluginName]
  );

  const pluginPageSource = useMemo(() => {
    if (!page) {
      return "";
    }

    const sourceUrl = new URL(apiService.resolveUrl(page.source_url));
    sourceUrl.searchParams.set("oxide_theme", theme);
    return sourceUrl.toString();
  }, [page, theme]);

  if (loading) {
    return (
      <PageLayout title="Plugin Page">
        <LoadingState label="Loading plugin page..." />
      </PageLayout>
    );
  }

  if (error) {
    return (
      <PageLayout title="Plugin Page">
        <ErrorState description={error} onRetry={fetchPages} />
      </PageLayout>
    );
  }

  if (!page || !page.enabled) {
    return (
      <PageLayout title="Plugin Page">
        <ErrorState
          title="Plugin page unavailable"
          description="The requested plugin page is not available."
          onRetry={fetchPages}
        />
      </PageLayout>
    );
  }

  return (
    <PageLayout
      title={page.title}
      description={page.description || `${page.plugin_name} ${page.plugin_version}`}
      leftActions={<Puzzle className="h-5 w-5 text-muted-foreground" />}
      headerActions={
        <Button type="button" variant="outline" size="sm" onClick={fetchPages}>
          <RefreshCw className="mr-2 h-4 w-4" />
          Refresh
        </Button>
      }
    >
      <iframe
        title={`${page.plugin_name}: ${page.title}`}
        src={pluginPageSource}
        className="h-[calc(100svh-10rem)] min-h-[520px] w-full rounded-md border bg-background"
        sandbox="allow-downloads allow-forms allow-popups allow-same-origin allow-scripts"
      />
    </PageLayout>
  );
};

export default PluginAdminPage;
