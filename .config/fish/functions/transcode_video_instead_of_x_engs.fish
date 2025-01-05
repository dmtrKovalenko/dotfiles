function transcode_video_instead_of_x_engs
    set source_video $argv[1]
    set destination (if set -q argv[2]; echo $argv[2]; else; echo "out.mp4"; end)


    if test -z "$source_video"
        echo "Missing required args. Usage: transcode_video_instead_of_x_engs \$source_video \$destination"    
        return 1
    end

    ffmpeg -i $source_video -r 30 -crf 18 -b:a 192k -ac 1 -c:v h264 -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,setsar=1" $destination

    ffplay $destination
end

