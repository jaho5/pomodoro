#!/bin/bash
echo "Downloading sample sound files..."

# Create the sounds directory if it doesn't exist
mkdir -p sounds

# Download sound files using curl - selecting longer sounds (3+ seconds)
curl -L "https://assets.mixkit.co/active_storage/sfx/2869/2869.wav" -o sounds/work_done.wav
curl -L "https://assets.mixkit.co/active_storage/sfx/2868/2868.wav" -o sounds/break_done.wav
curl -L "https://assets.mixkit.co/active_storage/sfx/1862/1862.wav" -o sounds/start.wav

echo "Sample sound files downloaded to the sounds directory."
echo "  work_done.wav - Plays when a work session is complete (Gentle marimba notification sound)"
echo "  break_done.wav - Plays when a break is complete (Achievement bell notification sound)"
echo "  start.wav - Plays when a work session starts (Happy bells notification sound)"

echo ""
echo "You can customize these sounds by replacing files in the 'sounds' directory."

# Make the script executable
chmod +x download_sounds.sh
