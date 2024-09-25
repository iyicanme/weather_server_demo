# weather_server_demo

Serves a weather information API that locates user from their IP address.

Makes use of `ipapi.co` for geolocation and `weatherapi.com` for weather information.

## Prerequisites
This program requires some configuration over two sources and some setup:

### Configuration file
A configuration file named: `config.toml` is required to be available on program start
in the current working directory.

Configuration file includes two entries:

`port` determines which port the server will serve on.

`database_name` determines what name the user database file should be.
Database name should not include paths or extensions.

### Environment variables
Program requires two environment variables to be set before start.

`JWT_SECRET` is used as the secret when issuing JWT tokens.

`WEATHER_API_KEY` is the API key for `weatherapi.com`.
An API key can be acquired by signing up at `https://www.weatherapi.com/signup.aspx` and
heading to `https://www.weatherapi.com/my/`.

These configurations are expected through environment variables so they can be set
when hosted cloud container services through their interfaces.

### Weather API response fields setup
`weatherapi.com` API is configured to send only the required information on API call.

The API can be configured at `https://www.weatherapi.com/my/fields.aspx`.
Under `Current Weather` section, only the fields: `last_updated`, `temp_c`, `text` and
`feels_like_c` should be selected.

## Running the project

Unless hosted in cloud services, program should be run in `dev` profile. 
Reason for that is, when the program is run locally and tested with local clients, the IP `weather` endpoint sees
is a local address, which geolocation IP responds with an invalid coordinate.
To avoid that and test the system locally, local IPs are replaced in dev and test profiles.

To run the project in dev profile, simply run it without `--release` flag.

```shell
$ cargo run
```
