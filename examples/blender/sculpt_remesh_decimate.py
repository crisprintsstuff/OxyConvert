bl_info = {
    "name": "Sculpt Remesh and Decimate",
    "author": "BlendArtist",
    "version": (1, 2, 0),
    "description": "Blender sculpt operator for automated voxel remesh and decimation",
    "category": "Sculpt",
}

import bpy

class SCULPT_OT_quick_remesh(bpy.types.Operator):
    bl_idname = "sculpt.quick_remesh"
    bl_label = "Quick Sculpt Remesh"

    def execute(self, context):
        context.object.data.remesh_voxel_size = 0.035
        bpy.ops.object.voxel_remesh()
        bpy.ops.object.modifier_add(type='DECIMATE')
        return {'FINISHED'}
