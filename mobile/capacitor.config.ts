import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'com.antigravity.coheara',
  appName: 'Coheara',
  webDir: 'build',
  server: {
    // In development, load from the Vite dev server for HMR
    // url: 'http://192.168.1.42:1421',
    // cleartext: true,
  },
  plugins: {
    SplashScreen: {
      launchAutoHide: true,
      launchShowDuration: 1500,
      backgroundColor: '#ffffff',
      showSpinner: false,
    },
    Keyboard: {
      resize: 'body',
      resizeOnFullScreen: true,
    },
    StatusBar: {
      style: 'DARK',
      backgroundColor: '#ffffff',
    },
  },
  android: {
    // Prevent screenshots of health data
    allowMixedContent: false,
  },
  ios: {
    // Capacitor iOS configuration
    contentInset: 'automatic',
  },
};

export default config;
