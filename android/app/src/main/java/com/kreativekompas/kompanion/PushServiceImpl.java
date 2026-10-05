package com.kreativekompas.kompanion;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.content.Intent;
import android.os.Bundle;
import android.app.PendingIntent;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

import org.unifiedpush.android.connector.PushService;
import org.unifiedpush.android.connector.FailedReason;
import org.unifiedpush.android.connector.data.PushEndpoint;
import org.unifiedpush.android.connector.data.PushMessage;

public class PushServiceImpl extends PushService {

    @Override
    public void onNewEndpoint(PushEndpoint endpoint, String instance) {
        Push.save(this, endpoint.getUrl());
        Push.sendIfNeeded(this);
    }

    @Override
    public void onMessage(PushMessage message, String instance) {
        String text = null;
        try {
            text = new String(message.getContent(), java.nio.charset.StandardCharsets.UTF_8);
            JSONObject json = new JSONObject(text);
            
            String title = json.optString("title", "Kompanion");
            String state = json.optString("state", "");
            String url = json.optString("url", "");

            String label;
            if ("needs you".equals(state)) {
                label = getString(R.string.state_needs_you);
            } else if ("failed".equals(state)) {
                label = getString(R.string.state_failed);
            } else if ("done".equals(state)) {
                label = getString(R.string.state_done);
            } else {
                label = state;
            }

            showNotification(title, label, url);
        } catch (JSONException e) {
            showNotification("Kompanion", text, "");
        }
    }

    @Override
    public void onRegistrationFailed(FailedReason reason, String instance) {
        Log.w("Kompanion", "push registration failed: " + reason);
    }

    @Override
    public void onUnregistered(String instance) {
        Push.clear(this);
    }

    private void showNotification(String title, String label, String url) {
        createNotificationChannel();

        Intent intent = new Intent(this, MainActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        
        Bundle extras = new Bundle();
        extras.putString("url", url);
        intent.putExtras(extras);

        PendingIntent pendingIntent;
        if (url.isEmpty()) {
            pendingIntent = PendingIntent.getActivity(this, 0, intent, 
                    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        } else {
            pendingIntent = PendingIntent.getActivity(this, url.hashCode(), intent, 
                    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        }

        Notification.Builder builder = new Notification.Builder(this, "tasks")
                .setSmallIcon(R.drawable.ic_stat)
                .setContentTitle(title)
                .setContentText(label)
                .setColor(0xFFF3941F)
                .setAutoCancel(true)
                .setContentIntent(pendingIntent);

        int notificationId;
        if (url.isEmpty()) {
            notificationId = (int) System.currentTimeMillis();
        } else {
            notificationId = url.hashCode();
        }

        NotificationManager notificationManager = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        notificationManager.notify(notificationId, builder.build());
    }

    private void createNotificationChannel() {
        NotificationChannel channel = new NotificationChannel(
                "tasks",
                getString(R.string.channel_tasks),
                NotificationManager.IMPORTANCE_HIGH
        );
        NotificationManager manager = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        manager.createNotificationChannel(channel);
    }
}
