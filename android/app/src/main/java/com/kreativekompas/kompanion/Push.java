package com.kreativekompas.kompanion;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Build;
import android.util.Log;
import android.webkit.CookieManager;
import org.json.JSONObject;

import java.io.DataOutputStream;
import java.net.HttpURLConnection;
import java.net.URL;

public final class Push {
    private static final String PREFS = "push";
    private static final String KEY_ENDPOINT = "endpoint";
    private static final String KEY_SENT = "sent";

    private Push() {
        // Private constructor to prevent instantiation
    }

    public static void save(Context c, String endpoint) {
        SharedPreferences prefs = c.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
        prefs.edit().putString(KEY_ENDPOINT, endpoint).apply();
    }

    public static void clear(Context c) {
        SharedPreferences prefs = c.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
        prefs.edit().remove(KEY_ENDPOINT).remove(KEY_SENT).apply();
    }

    public static void sendIfNeeded(Context c) {
        new Thread(() -> {
            try {
                SharedPreferences prefs = c.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
                String endpoint = prefs.getString(KEY_ENDPOINT, null);
                if (endpoint == null || endpoint.isEmpty()) {
                    return;
                }

                String sentEndpoint = prefs.getString(KEY_SENT, null);
                if (sentEndpoint != null && sentEndpoint.equals(endpoint)) {
                    return;
                }

                String server = c.getString(R.string.server_url);
                if (server == null || server.isEmpty()) {
                    return;
                }

                CookieManager cookieManager = CookieManager.getInstance();
                cookieManager.setAcceptCookie(true);
                String cookie = cookieManager.getCookie(server);

                if (cookie == null || cookie.isEmpty()) {
                    return;
                }

                URL url = new URL(server + "/api/push/register");
                HttpURLConnection connection = (HttpURLConnection) url.openConnection();
                
                try {
                    connection.setConnectTimeout(10000);
                    connection.setReadTimeout(10000);
                    connection.setRequestMethod("POST");
                    connection.setRequestProperty("Content-Type", "application/json");
                    connection.setRequestProperty("X-Kompanion", "1");
                    connection.setRequestProperty("Origin", server);
                    connection.setRequestProperty("Cookie", cookie);
                    connection.setDoOutput(true);

                    JSONObject body = new JSONObject();
                    body.put("endpoint", endpoint);
                    body.put("device", Build.MANUFACTURER + " " + Build.MODEL);

                    DataOutputStream dos = new DataOutputStream(connection.getOutputStream());
                    dos.write(body.toString().getBytes("UTF-8"));
                    dos.flush();
                    dos.close();

                    int responseCode = connection.getResponseCode();
                    if (responseCode == 204) {
                        SharedPreferences prefs2 = c.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
                        prefs2.edit()
                          .putString(KEY_SENT, endpoint)
                          .apply();
                    }
                } finally {
                    connection.disconnect();
                }
            } catch (Exception e) {
                Log.w("Kompanion", "push register failed: " + e.getClass().getSimpleName());
            }
        }).start();
    }
}
