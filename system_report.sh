#!/bin/bash

# Function to check internet connectivity
check_connectivity() {
    echo "Checking internet connectivity..."
    # Using the local tempest_ai network_check tool via raw shell if needed, 
    # but here we use standard ping for reliability in a standalone script.
    ping -c 3 8.8.8.8
}

# Function to get system information
get_system_info() {
    echo "Collecting system information..."
    uname -a
    sw_vers
    top -l 1 | head -n 10
}

# Function to update Homebrew and packages
update_homebrew() {
    echo "Updating Homebrew and packages..."
    brew update
    brew upgrade
}

# Run the functions and save the output to a file
output_file="system_report.txt"

{
  check_connectivity
  echo "-------------------"
  get_system_info
  echo "-------------------"
  update_homebrew
} > "$output_file" 2>&1

echo "Report saved to $output_file"
