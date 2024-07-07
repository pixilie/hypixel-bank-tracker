{ pkgs
, lib
, config
, inputs
, ... }:

{
  packages = with pkgs; [ bun ];
}
