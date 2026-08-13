# Core concepts

- **Location** - A logical grouping of members. Each has an activity log that keeps track of members who sign in and out at the location. Usually a single physical location but can have multiple or no physical location.
- **Member** - A person who can sign in and out at a sign in kiosk.
- **Kiosk** - A computer or similar (tablet) often with a touchscreen, keyboard and/or mouse that is registered to use a SES Activity "session" which securely registers it to a location. It provides an interface that allows members to sign in and out, select a category that best represents their time and modify their sign in and out times. A location can have more than one kiosk and some may perform tasks other than sign in and out.
- **Session** - A record unit administrators can create that provides a 6 digit code that can be used to set up a kiosk and tie it to a location.
- **Location administrator** or **User** - A user who can login and administer one or more locations in the SES Activity system. Users cannot sign in and out at a kiosk by themselves. Most users will also have a member record at one location that they can use to sign in and out at kiosks.
- **"Scan"** - The name given to the part of the website that offers the kiosk functionality. Likely to be renamed "Kiosk" in future.
- **"Dash"** - The administrator's dashboard - the part of the website that offers administration functionality to Location administrators/Users.

# Kiosk requirements

To take advantage of SES Activity's easy sign-in/sign-out functionality for members you need to provide a computer that the system can run on. Here are some requirements and considerations:

- Computer must run a modern web browser such as Chrome, Edge or Safari.
- You should be able to configure it to stay on so it is ready for use by members at any time.
- It should not log out when idle or require login after a power outage.
- It should be able to be configured to open to the SES Login webpage at startup so it is ready to go after a power outage.
- You need to either provide a touchscreen or you need to ensure a mouse and keyboard is available. A numpad only keyboard should work fine.
- Internet connection should be reasonably stable. The system is fairly fault tolerant but a flaky connection will reduce the speed at which people can sign in and out.

# Getting started

- Contact a maintainer of the system to gain access. Provide them your unit name and the email addresses of anyone you'd like to help administer your unit (suggested maximum of 3 to start).
- Once your accounts have been created visit the website and click Dash. Follow the prompts to sign in by typing your email exactly as it was provided in the previous step. Visit your email to get the 6 digit code to sign in.
- Once you are on the admin dashboard for your unit visit the members list to create a member (suggest you start by creating a record for yourself). You can skip this step if members have been automatically synced.
- Now visit the Sessions tab to create a Session for your kiosk. Create one. This will give you a 6 digit code you can use to provision your kiosk. Keep the tab open or write the code down.
- On the computer you want to set up as a kiosk visit the seslogin website and click Scan or Kiosk. If you're just trying out the system you can do this in a new browser tab on your computer.
- Enter the 6 digit code from earlier.
- Your kiosk should now be ready to use. You can test it by entering your member ID (assuming your record has synced or you created it earlier). The first time you enter your member ID and press return at a location you will be signed in. When you enter it again you will be prompted for a time category and given the ability to edit it before you are then signed out.
- If you are not just testing you should save the current web address as a bookmark/favourite and/or set it as the homepage so you can get back to it easily. If the webpage does not stay open the session will expire after two weeks and you'll need to set it back up again. A kiosk set up by scanning a QR code instead of typing a 6 digit code doesn't have to be set up from scratch: switch it back on, wait for it to show its QR code screen, then press Reactivate next to it on the Sessions tab.

# Kiosk set-up tips

- [Here's a decent guide for setting up a Chrome kiosk on Windows](https://superuser.com/a/1363172)
- RaspberryPi - recommend trying [FullPageOS](https://github.com/guysoft/FullPageOS/)

# Setting up multiple screens on one kiosk

TODO

# Frequently asked questions

- **Can a member from location A sign in at location B?** - Yes, any member of any unit can sign in and out at any kiosk. Their time entries will be entered on that location's activity log. It will also be shown on their personal activity log.

- **Can I have more than one kiosk set up within a location?** - Yes you can have multiple. Members do not need to sign in and out at the same computer - they can sign in on one and out on another. It will be correctly counted as one time entry at the location.

- **Can I tell the difference between a member that has signed in at one kiosk within a location vs that same member signing in at a different kiosk within the same location?** - No, currently there is no support for this. The time entries would look identical as they would show the same location.

- **What happens if I sign in on a kiosk at one location and then attempt to sign out at another location?** - Both actions would result in sign-ins as time entries cannot span locations. The member would effectively be signed in in multiple locations. Entering their member ID on the kiosk at either of these locations should result in closure (ie sign out) of the time entry at that location.
