// Name: Nomad Organic Sculpt Pass
// Author: MobileArtist
// Description: Configures clay brush, runs organic remesh and decimation
nomad.setBrush('clay');
nomad.symmetry = true;
nomad.topology.remesh(0.03);
nomad.decimate(30);
console.log("Nomad organic pass executed");
