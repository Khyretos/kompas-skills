package com.kreativekompas.kompanion;

import android.Manifest;
import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Bundle;
import android.webkit.CookieManager;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Toast;
import java.util.Arrays;
import org.unifiedpush.android.connector.UnifiedPush;
import kotlin.Unit;

public class MainActivity extends Activity {
    private WebView web;
    private String server;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);
        server = getString(R.string.server_url);
        web = findViewById(R.id.web);

        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setMediaPlaybackRequiresUserGesture(false);

        CookieManager.getInstance().setAcceptCookie(true);
        CookieManager.getInstance().setAcceptThirdPartyCookies(web, false);

        web.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest r) {
                String host = r.getUrl().getHost();
                if (Arrays.asList(getResources().getStringArray(R.array.inside_hosts)).contains(host)) {
                    return false;
                } else {
                    try {
                        startActivity(new Intent(Intent.ACTION_VIEW, r.getUrl()));
                    } catch (ActivityNotFoundException e) {
                        // ignore
                    }
                    return true;
                }
            }

            @Override
            public void onPageFinished(WebView v, String url) {
                if (url.startsWith(server)) {
                    CookieManager.getInstance().flush();
                    Push.sendIfNeeded(MainActivity.this);
                }
            }
        });

        web.setWebChromeClient(new WebChromeClient());

        if (savedInstanceState != null) {
            web.restoreState(savedInstanceState);
        } else {
            web.loadUrl(startUrl(getIntent()));
        }

        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] { Manifest.permission.POST_NOTIFICATIONS }, 1);
        }

        UnifiedPush.tryUseCurrentOrDefaultDistributor(this, ok -> {
            if (ok) {
                UnifiedPush.register(MainActivity.this, "default", getString(R.string.app_name), null);
            } else {
                runOnUiThread(() -> Toast.makeText(MainActivity.this, R.string.no_distributor, Toast.LENGTH_LONG).show());
            }
            return Unit.INSTANCE;
        });
    }

    private String startUrl(Intent i) {
        String u = i == null ? null : i.getStringExtra("url");
        return (u != null && u.startsWith(server + "/")) ? u : server + "/";
    }

    @Override
    protected void onNewIntent(Intent i) {
        super.onNewIntent(i);
        setIntent(i);
        String u = i.getStringExtra("url");
        if (u != null && u.startsWith(server + "/")) {
            web.loadUrl(u);
        }
    }

    @Override
    protected void onSaveInstanceState(Bundle b) {
        super.onSaveInstanceState(b);
        web.saveState(b);
    }

    @SuppressWarnings("deprecation")
    @Override
    public void onBackPressed() {
        if (web.canGoBack()) {
            web.goBack();
        } else {
            super.onBackPressed();
        }
    }

    @Override
    protected void onPause() {
        super.onPause();
        CookieManager.getInstance().flush();
    }
}
